use crate::config::{LogLevel, PreprocessingMode};
use crate::{log_message, start_timer, stop_timer};
use geo::algorithm::bounding_rect::BoundingRect;
use image::{imageops, GenericImageView, GrayImage};
use std::borrow::Cow;

const SCALED_IMAGE_HEIGHT: u32 = 1000;

/// Debug information for visualizing the image processing pipeline
pub struct DebugInfo {
    pub blurred: GrayImage,
    pub edges: GrayImage,
    pub dilated: GrayImage,
    pub closed: GrayImage,
    pub cleaned: GrayImage,
    pub all_contours: Vec<(geo::Rect<u32>, f64)>, // (rect, rectangularity score)
}

/// Information about a detected photo including its bounding box and rotation
#[derive(Clone, Debug)]
pub struct DetectedPhoto {
    pub rect: geo::Rect<u32>,
    pub rotation_angle: f64, // in degrees, positive = clockwise
}

/// Result of edge detection with optional debug info
pub struct DetectionResult {
    pub photos: Vec<DetectedPhoto>,
    pub debug_info: Option<DebugInfo>,
}

/// Merge overlapping or nearby bounding boxes
fn merge_overlapping_rects(rects: Vec<geo::Rect<u32>>) -> Vec<geo::Rect<u32>> {
    if rects.len() <= 1 {
        return rects;
    }

    let mut merged = Vec::new();
    let mut used = vec![false; rects.len()];

    for i in 0..rects.len() {
        if used[i] {
            continue;
        }

        let mut current = rects[i];
        used[i] = true;
        let mut merged_any = true;

        // Keep trying to merge until no more overlaps found
        while merged_any {
            merged_any = false;
            for j in 0..rects.len() {
                if used[j] {
                    continue;
                }

                // Check if rectangles overlap or are very close (within 20% of smaller dimension)
                // Use the smaller dimension between BOTH rectangles for tolerance calculation
                let current_min_dim = current.width().min(current.height());
                let other_min_dim = rects[j].width().min(rects[j].height());
                let tolerance = ((current_min_dim.min(other_min_dim) as f64) * 0.1) as u32;
                
                let current_expanded = geo::Rect::new(
                    geo::Coord {
                        x: current.min().x.saturating_sub(tolerance),
                        y: current.min().y.saturating_sub(tolerance),
                    },
                    geo::Coord {
                        x: current.max().x + tolerance,
                        y: current.max().y + tolerance,
                    },
                );

                // Check for intersection
                let intersects = !(current_expanded.max().x < rects[j].min().x
                    || current_expanded.min().x > rects[j].max().x
                    || current_expanded.max().y < rects[j].min().y
                    || current_expanded.min().y > rects[j].max().y);

                if intersects {
                    // Merge: create union of both rectangles
                    let min_x = current.min().x.min(rects[j].min().x);
                    let min_y = current.min().y.min(rects[j].min().y);
                    let max_x = current.max().x.max(rects[j].max().x);
                    let max_y = current.max().y.max(rects[j].max().y);

                    current = geo::Rect::new(
                        geo::Coord { x: min_x, y: min_y },
                        geo::Coord { x: max_x, y: max_y },
                    );
                    used[j] = true;
                    merged_any = true;

                    log_message!(
                        LogLevel::Debug,
                        &format!(
                            "Merged overlapping boxes: ({},{} {}x{}) + ({},{} {}x{}) -> ({},{} {}x{})",
                            rects[i].min().x, rects[i].min().y, rects[i].width(), rects[i].height(),
                            rects[j].min().x, rects[j].min().y, rects[j].width(), rects[j].height(),
                            current.min().x, current.min().y, current.width(), current.height()
                        )
                    );
                }
            }
        }

        merged.push(current);
    }

    merged
}

/// Calculate rotation angle of a contour using minimum area bounding rectangle
/// Returns angle in degrees (positive = clockwise rotation needed to straighten)
fn calculate_rotation_angle(contour: &imageproc::contours::Contour<u32>) -> f64 {
    if contour.points.len() < 4 {
        return 0.0;
    }

    let points = &contour.points;
    
    // Find extreme points (top-left, top-right, bottom-left, bottom-right)
    // by looking at corners of the axis-aligned bounding box
    let min_x = points.iter().map(|p| p.x).min().unwrap();
    let max_x = points.iter().map(|p| p.x).max().unwrap();
    let min_y = points.iter().map(|p| p.y).min().unwrap();
    let max_y = points.iter().map(|p| p.y).max().unwrap();
    
    // For each corner region, find the actual contour point closest to that corner
    // Top-left region
    let tl = points.iter()
        .filter(|p| p.x < min_x + (max_x - min_x) / 3 && p.y < min_y + (max_y - min_y) / 3)
        .min_by_key(|p| (p.x as i64 - min_x as i64).pow(2) + (p.y as i64 - min_y as i64).pow(2))
        .or_else(|| points.iter().min_by_key(|p| (p.x as i64 - min_x as i64).pow(2) + (p.y as i64 - min_y as i64).pow(2)));
    
    // Top-right region
    let tr = points.iter()
        .filter(|p| p.x > max_x - (max_x - min_x) / 3 && p.y < min_y + (max_y - min_y) / 3)
        .min_by_key(|p| (p.x as i64 - max_x as i64).pow(2) + (p.y as i64 - min_y as i64).pow(2))
        .or_else(|| points.iter().min_by_key(|p| (p.x as i64 - max_x as i64).pow(2) + (p.y as i64 - min_y as i64).pow(2)));
    
    // Bottom-left region
    let bl = points.iter()
        .filter(|p| p.x < min_x + (max_x - min_x) / 3 && p.y > max_y - (max_y - min_y) / 3)
        .min_by_key(|p| (p.x as i64 - min_x as i64).pow(2) + (p.y as i64 - max_y as i64).pow(2))
        .or_else(|| points.iter().min_by_key(|p| (p.x as i64 - min_x as i64).pow(2) + (p.y as i64 - max_y as i64).pow(2)));
    
    // Bottom-right region  
    let br = points.iter()
        .filter(|p| p.x > max_x - (max_x - min_x) / 3 && p.y > max_y - (max_y - min_y) / 3)
        .min_by_key(|p| (p.x as i64 - max_x as i64).pow(2) + (p.y as i64 - max_y as i64).pow(2))
        .or_else(|| points.iter().min_by_key(|p| (p.x as i64 - max_x as i64).pow(2) + (p.y as i64 - max_y as i64).pow(2)));
    
    // Calculate angles from edges if we found valid corners
    if let (Some(tl), Some(tr), Some(bl), Some(br)) = (tl, tr, bl, br) {
        // Calculate angle from top edge (most reliable for scanned photos)
        let top_dx = tr.x as f64 - tl.x as f64;
        let top_dy = tr.y as f64 - tl.y as f64;
        let top_angle = top_dy.atan2(top_dx).to_degrees();
        
        // Calculate angle from bottom edge
        let bottom_dx = br.x as f64 - bl.x as f64;
        let bottom_dy = br.y as f64 - bl.y as f64;
        let bottom_angle = bottom_dy.atan2(bottom_dx).to_degrees();
        
        // Average the two angles for more stability
        let avg_angle = (top_angle + bottom_angle) / 2.0;
        
        // Normalize to [-45, 45] range (since rectangles have 90° symmetry)
        let normalized_angle = if avg_angle > 45.0 {
            avg_angle - 90.0
        } else if avg_angle < -45.0 {
            avg_angle + 90.0
        } else {
            avg_angle
        };
        
        // Return negative angle (since we want the angle to rotate back to upright)
        -normalized_angle
    } else {
        0.0
    }
}

/// Calculate how rectangular a contour is (0.0 = not rectangular, 1.0 = perfect rectangle)
fn calculate_rectangularity(contour: &imageproc::contours::Contour<u32>, bounding_rect: &geo::Rect<u32>) -> f64 {
    let contour_area = contour.points.len() as f64;
    
    // For a perfect rectangle, the contour perimeter should be close to the bounding box perimeter
    // We approximate this by comparing areas (contour pixels vs bounding box area)
    // A good rectangle should have a contour that outlines the bounding box well
    
    let rect_perimeter = 2.0 * (bounding_rect.width() + bounding_rect.height()) as f64;
    
    // Rectangularity: contour should have points roughly equal to the perimeter
    // but not too many more (which would indicate a complex shape)
    let perimeter_ratio = contour_area / rect_perimeter;
    
    // Good rectangles have ratio between 0.5 and 3.0
    // (exact perimeter would be 1.0, but edges may be slightly jagged)
    if perimeter_ratio < 0.3 || perimeter_ratio > 5.0 {
        return 0.0;
    }
    
    // Also check aspect ratio - photos shouldn't be too elongated (like a line)
    let aspect_ratio = bounding_rect.width() as f64 / bounding_rect.height() as f64;
    let normalized_aspect = aspect_ratio.max(1.0 / aspect_ratio);
    
    // Reject very elongated shapes (aspect ratio > 10:1 suggests a line, not a photo)
    if normalized_aspect > 10.0 {
        return 0.0;
    }
    
    // Calculate score based on how reasonable the perimeter ratio is
    let perimeter_score = if perimeter_ratio < 1.0 {
        perimeter_ratio / 0.3
    } else if perimeter_ratio < 3.0 {
        1.0
    } else {
        (5.0 - perimeter_ratio) / 2.0
    };
    
    // Calculate score based on aspect ratio (prefer more square-like shapes)
    let aspect_score = 1.0 / normalized_aspect.max(1.0);
    
    // Combine scores (weighted average)
    0.7 * perimeter_score + 0.3 * aspect_score
}

pub fn process_image(img: &image::DynamicImage, preprocess_mode: PreprocessingMode) -> Result<(GrayImage, f64), String> {
    let (scaling_factor, resized_img) = if preprocess_mode == PreprocessingMode::NetworkOptimized {
        let (width, height) = img.dimensions();
        let scaling_factor = (SCALED_IMAGE_HEIGHT as f64) / height as f64;
        let new_width = (width as f64 * scaling_factor) as u32;

        let start_time = start_timer!("Image resizing");
        let resized = img.resize(
            new_width,
            SCALED_IMAGE_HEIGHT,
            imageops::FilterType::Triangle,
        );
        stop_timer!(start_time, "Image resizing");
        (scaling_factor, Cow::Owned(resized))
    } else {
        (1.0, Cow::Borrowed(img))
    };

    let start_time = start_timer!("Converting image to grayscale");
    let grayscale_img = resized_img.to_luma8();
    stop_timer!(start_time, "Converting image to grayscale");

    Ok((grayscale_img, scaling_factor))
}

pub fn detect_edges(_image: &GrayImage, debug_enabled: bool) -> DetectionResult {
    detect_edges_with_options(_image, debug_enabled, true)
}

pub fn detect_edges_with_options(_image: &GrayImage, debug_enabled: bool, filter_oversized: bool) -> DetectionResult {
    let step_time = start_timer!("Applying Gaussian blur");
    let blurred_img = imageproc::filter::gaussian_blur_f32(&_image, 2.5);
    stop_timer!(step_time, "Applying Gaussian blur");

    let step_time = start_timer!("Canny edge detection");
    // Lower thresholds to detect more edges, especially for photos with similar background colors
    let edges = imageproc::edges::canny(&blurred_img, 5.0, 40.0);
    stop_timer!(step_time, "Canny edge detection");

    let step_time = start_timer!("Morphological operations");
    // Use morphological closing: dilate then erode with SAME iterations
    // This fills holes in boundaries while maintaining overall shape
    let closing_iterations = 25;
    
    // Dilate to connect nearby edges and close gaps in photo boundaries
    let dilated = imageproc::morphology::dilate(&edges, imageproc::distance_transform::Norm::L1, closing_iterations);
    
    // Erode by same amount to restore original size while keeping holes filled
    // let closed = imageproc::morphology::close(&edges, imageproc::distance_transform::Norm::L1, closing_iterations);
    let closed = imageproc::morphology::erode(&dilated, imageproc::distance_transform::Norm::L1, closing_iterations);
    
    // Additional small dilation to ensure boundaries are slightly expanded for better capture
    let cleaned = imageproc::morphology::dilate(&closed, imageproc::distance_transform::Norm::L1, 2);
    stop_timer!(step_time, "Morphological operations");

    let step_time = start_timer!("Find contours");
    let contours: Vec<imageproc::contours::Contour<u32>> =
        imageproc::contours::find_contours_with_threshold(&cleaned, 128);
    stop_timer!(step_time, "Find contours");

    log_message!(
        LogLevel::Debug,
        &format!("Found {} contours", contours.len())
    );

    let mut detected_photos: Vec<(geo::Rect<u32>, f64, &imageproc::contours::Contour<u32>)> = vec![]; // (rect, rectangularity, contour)
    let mut potential_candidates: Vec<(geo::Rect<u32>, f64, &imageproc::contours::Contour<u32>)> = vec![]; // mid-sized candidates
    let mut all_contours_debug: Vec<(geo::Rect<u32>, f64)> = vec![];
    let image_area = _image.width() * _image.height();
    let min_area = (image_area as f32 * 0.05) as u32;
    let candidate_min_area = (image_area as f32 * 0.02) as u32; // Lower threshold for candidates
    let min_rectangularity = 0.3; // Minimum score to be considered a rectangle
    let candidate_min_rectangularity = 0.2; // Lower threshold for candidates
    let mut filtered_count = 0;

    for contour in &contours {
        // Skip hole contours - we only want outer boundaries
        if matches!(contour.border_type, imageproc::contours::BorderType::Hole) {
            continue;
        }


        let points = geo::MultiPoint(
            contour
                .points
                .iter()
                .map(|p| geo::Point::new(p.x, p.y))
                .collect(),
        );

        let bounding_rect = points.bounding_rect().unwrap();
        let rect_area = bounding_rect.width() * bounding_rect.height();

        // Calculate how rectangular this contour is
        let rectangularity = calculate_rectangularity(&contour, &bounding_rect);

        // Store all contours for debug visualization
        if debug_enabled {
            all_contours_debug.push((bounding_rect, rectangularity));
        }

        let min_dimension = 50; // Minimum width or height in pixels
        
        // Check if this meets the high threshold for detected photos
        let is_detected = rect_area >= min_area 
            && rectangularity >= min_rectangularity
            && bounding_rect.width() >= min_dimension
            && bounding_rect.height() >= min_dimension;

        // Check if this meets the lower threshold for potential candidates
        let is_candidate = !is_detected 
            && rect_area >= candidate_min_area
            && rect_area < min_area  // Must be mid-sized (between candidate and detected thresholds)
            && rectangularity >= candidate_min_rectangularity
            && bounding_rect.width() >= min_dimension
            && bounding_rect.height() >= min_dimension;

        if is_detected {
            detected_photos.push((bounding_rect, rectangularity, contour));

            log_message!(
                LogLevel::Debug,
                &format!(
                    "Accepted box: x={}, y={}, w={}, h={} (area: {}, rectangularity: {:.2})",
                    bounding_rect.min().x,
                    bounding_rect.min().y,
                    bounding_rect.width(),
                    bounding_rect.height(),
                    rect_area,
                    rectangularity
                )
            );
        } else if is_candidate {
            potential_candidates.push((bounding_rect, rectangularity, contour));

            log_message!(
                LogLevel::Debug,
                &format!(
                    "Potential candidate: x={}, y={}, w={}, h={} (area: {}, rectangularity: {:.2})",
                    bounding_rect.min().x,
                    bounding_rect.min().y,
                    bounding_rect.width(),
                    bounding_rect.height(),
                    rect_area,
                    rectangularity
                )
            );
        } else {
            filtered_count += 1;
        }
    }

    // Log summary
    log_message!(
        LogLevel::Debug,
        &format!("Found {} detected photos, {} potential candidates", detected_photos.len(), potential_candidates.len())
    );
    if filtered_count > 0 {
        log_message!(
            LogLevel::Debug,
            &format!("Filtered out {} contours (too small, wrong shape, or low rectangularity)", filtered_count)
        );
    }

    // Filter out bounding boxes that are too large (more than 95% of image)
    // Skip this filter for recursive processing
    if filter_oversized {
        let max_area = (image_area as f32 * 0.95) as u32;
        let original_count = detected_photos.len();
        detected_photos.retain(|(rect, _, _)| {
            let area = rect.width() * rect.height();
            let is_valid = area <= max_area;
            if !is_valid {
                log_message!(
                    LogLevel::Debug,
                    &format!(
                        "Filtered out oversized box: x={}, y={}, w={}, h={} (area: {}, {}% of image)",
                        rect.min().x,
                        rect.min().y,
                        rect.width(),
                        rect.height(),
                        area,
                        (area as f32 / image_area as f32 * 100.0) as u32
                    )
                );
            }
            is_valid
        });
        if original_count != detected_photos.len() {
            log_message!(
                LogLevel::Debug,
                &format!("Filtered out {} oversized boxes", original_count - detected_photos.len())
            );
        }
    }

    // Filter out bounding boxes that touch the image borders
    let border_margin = 5; // Allow small margin for edge detection noise
    let original_count = detected_photos.len();
    detected_photos.retain(|(rect, _, _)| {
        let touches_border = rect.min().x <= border_margin
            || rect.min().y <= border_margin
            || rect.max().x >= _image.width() - border_margin
            || rect.max().y >= _image.height() - border_margin;
        
        if touches_border {
            log_message!(
                LogLevel::Debug,
                &format!(
                    "Filtered out border-touching detected box: x={}, y={}, w={}, h={}",
                    rect.min().x,
                    rect.min().y,
                    rect.width(),
                    rect.height()
                )
            );
        }
        !touches_border
    });
    if original_count != detected_photos.len() {
        log_message!(
            LogLevel::Debug,
            &format!("Filtered out {} border-touching detected boxes", original_count - detected_photos.len())
        );
    }

    // Also filter out border-touching candidates
    let original_count = potential_candidates.len();
    potential_candidates.retain(|(rect, _, _)| {
        let touches_border = rect.min().x <= border_margin
            || rect.min().y <= border_margin
            || rect.max().x >= _image.width() - border_margin
            || rect.max().y >= _image.height() - border_margin;
        !touches_border
    });
    if original_count != potential_candidates.len() {
        log_message!(
            LogLevel::Debug,
            &format!("Filtered out {} border-touching candidates", original_count - potential_candidates.len())
        );
    }

    // Combine detected photos with candidates that are near detected photos
    let mut all_boxes = detected_photos.clone();
    let mut included_candidates = 0;
    
    for candidate in &potential_candidates {
        let candidate_rect = candidate.0;
        
        // Check if this candidate is near any detected photo
        let is_near_detected = detected_photos.iter().any(|(detected_rect, _, _)| {
            // Check proximity (within 20% of the smaller box's dimensions)
            let proximity_margin = ((detected_rect.width().min(detected_rect.height()) as f64) * 0.2) as u32;
            
            let detected_expanded = geo::Rect::new(
                geo::Coord {
                    x: detected_rect.min().x.saturating_sub(proximity_margin),
                    y: detected_rect.min().y.saturating_sub(proximity_margin),
                },
                geo::Coord {
                    x: detected_rect.max().x + proximity_margin,
                    y: detected_rect.max().y + proximity_margin,
                },
            );
            
            // Check for proximity/overlap
            !(detected_expanded.max().x < candidate_rect.min().x
                || detected_expanded.min().x > candidate_rect.max().x
                || detected_expanded.max().y < candidate_rect.min().y
                || detected_expanded.min().y > candidate_rect.max().y)
        });
        
        if is_near_detected {
            all_boxes.push(candidate.clone());
            included_candidates += 1;
            log_message!(
                LogLevel::Debug,
                &format!(
                    "Including nearby candidate: x={}, y={}, w={}, h={}",
                    candidate_rect.min().x,
                    candidate_rect.min().y,
                    candidate_rect.width(),
                    candidate_rect.height()
                )
            );
        }
    }
    
    log_message!(
        LogLevel::Debug,
        &format!("Including {} candidates near detected photos", included_candidates)
    );

    // Extract just the rectangles for merging
    let rects: Vec<geo::Rect<u32>> = all_boxes.iter().map(|(rect, _, _)| *rect).collect();
    
    // Merge overlapping or nearby bounding boxes
    log_message!(
        LogLevel::Debug,
        &format!("Merging {} rectangles (detected + nearby candidates)...", rects.len())
    );
    let merged_rects = merge_overlapping_rects(rects);
    log_message!(
        LogLevel::Debug,
        &format!("After merging: {} rectangles", merged_rects.len())
    );

    // Calculate rotation angles for merged rectangles
    // For each merged rect, find the best matching original contour and calculate its rotation
    let mut photos = Vec::new();
    for merged_rect in merged_rects {
        // Find the contour that best matches this merged rectangle (from all_boxes)
        let best_match = all_boxes.iter()
            .filter(|(rect, _, _)| {
                // Check if this original rect contributed to the merged rect
                let intersects = !(merged_rect.max().x < rect.min().x
                    || merged_rect.min().x > rect.max().x
                    || merged_rect.max().y < rect.min().y
                    || merged_rect.min().y > rect.max().y);
                intersects
            })
            .max_by_key(|(rect, _, _)| rect.width() * rect.height());
        
        let rotation_angle = if let Some((_, _, contour)) = best_match {
            let angle = calculate_rotation_angle(contour);
            
            // Only apply rotation if it's significant (> 1 degree)
            if angle.abs() > 1.0 {
                log_message!(
                    LogLevel::Debug,
                    &format!(
                        "Detected rotation: {:.1}° for box at ({},{})",
                        angle,
                        merged_rect.min().x,
                        merged_rect.min().y
                    )
                );
                angle
            } else {
                0.0
            }
        } else {
            0.0
        };
        
        photos.push(DetectedPhoto {
            rect: merged_rect,
            rotation_angle,
        });
    }

    let debug_info = if debug_enabled {
        Some(DebugInfo {
            blurred: blurred_img,
            edges,
            dilated,
            closed,
            cleaned,
            all_contours: all_contours_debug,
        })
    } else {
        None
    };

    DetectionResult {
        photos,
        debug_info,
    }
}
