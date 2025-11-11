use crate::config::{LogLevel, ProcessingConfig, ProcessingResult};
use crate::image_ops::{detect_edges, process_image};
use crate::log_message;
use std::path::Path;
use image::GenericImageView;

/// Rotate an image by the specified angle (in degrees, positive = clockwise)
fn rotate_image(img: image::DynamicImage, angle_degrees: f64) -> image::DynamicImage {
    use image::imageops;
    
    // Normalize angle to [-180, 180]
    let angle = angle_degrees % 360.0;
    let angle = if angle > 180.0 { angle - 360.0 } else if angle < -180.0 { angle + 360.0 } else { angle };
    
    // For angles close to 90 degree increments, use fast rotation
    if (angle - 90.0).abs() < 1.0 {
        return image::DynamicImage::ImageRgba8(imageops::rotate90(&img.to_rgba8()));
    } else if (angle - 180.0).abs() < 1.0 || (angle + 180.0).abs() < 1.0 {
        return image::DynamicImage::ImageRgba8(imageops::rotate180(&img.to_rgba8()));
    } else if (angle - 270.0).abs() < 1.0 || (angle + 90.0).abs() < 1.0 {
        return image::DynamicImage::ImageRgba8(imageops::rotate270(&img.to_rgba8()));
    }
    
    // For arbitrary angles, we need to do a proper rotation with interpolation
    // This is a simplified version - for production you'd want imageproc::geometric_transformations
    // For now, use a rotation approximation
    let angle_rad = -angle.to_radians(); // Negative because image coordinates
    let (width, height) = img.dimensions();
    let center_x = width as f64 / 2.0;
    let center_y = height as f64 / 2.0;
    
    // Calculate new image dimensions to fit rotated image
    let cos_a = angle_rad.cos().abs();
    let sin_a = angle_rad.sin().abs();
    let new_width = ((width as f64 * cos_a) + (height as f64 * sin_a)).ceil() as u32;
    let new_height = ((height as f64 * cos_a) + (width as f64 * sin_a)).ceil() as u32;
    
    let mut rotated = image::RgbaImage::from_pixel(new_width, new_height, image::Rgba([255, 255, 255, 255]));
    let img_rgba = img.to_rgba8();
    
    let new_center_x = new_width as f64 / 2.0;
    let new_center_y = new_height as f64 / 2.0;
    
    // Rotate each pixel (using inverse mapping for better quality)
    for y in 0..new_height {
        for x in 0..new_width {
            let dx = x as f64 - new_center_x;
            let dy = y as f64 - new_center_y;
            
            // Apply inverse rotation
            let src_x = (dx * angle_rad.cos() - dy * angle_rad.sin() + center_x).round() as i32;
            let src_y = (dx * angle_rad.sin() + dy * angle_rad.cos() + center_y).round() as i32;
            
            if src_x >= 0 && src_x < width as i32 && src_y >= 0 && src_y < height as i32 {
                rotated.put_pixel(x, y, *img_rgba.get_pixel(src_x as u32, src_y as u32));
            }
        }
    }
    
    image::DynamicImage::ImageRgba8(rotated)
}

pub struct ImageProcessor {
    config: ProcessingConfig,
}

impl ImageProcessor {
    pub fn new(config: ProcessingConfig) -> Self {
        ImageProcessor { config }
    }

    pub fn process_file(&self, input_path: &str) -> Result<ProcessingResult, String> {
        let img = image::open(input_path).map_err(|e| format!("Failed to load image: {}", e))?;
        let (processed_img, scaling_factor) =
            process_image(&img, self.config.preprocess_mode.clone())?;

        log_message!(LogLevel::Info, &format!("Image processing completed for {}", input_path));

        let detection_result = detect_edges(&processed_img, self.config.debug_enabled);
        let detected_photos = &detection_result.photos;
        let mut output_files = Vec::new();

        for (i, photo) in detected_photos.iter().enumerate() {
            let rect = &photo.rect;
            let rotation_angle = photo.rotation_angle;
            
            let scaled_x = (rect.min().x as f64 / scaling_factor).floor() as u32;
            let scaled_y = (rect.min().y as f64 / scaling_factor).floor() as u32;
            let scaled_w = (rect.width() as f64 / scaling_factor).ceil() as u32;
            let scaled_h = (rect.height() as f64 / scaling_factor).ceil() as u32;

            if scaled_x + scaled_w > img.width() || scaled_y + scaled_h > img.height() {
                return Err("Crop rectangle exceeds image bounds".to_string());
            }

            let mut cropped_img = img.crop_imm(scaled_x, scaled_y, scaled_w, scaled_h);
            
            // Apply rotation if needed
            if rotation_angle.abs() > 0.5 {
                cropped_img = rotate_image(cropped_img, rotation_angle);
                log_message!(
                    LogLevel::Debug,
                    &format!("Rotated image {} by {:.1}°", i + 1, rotation_angle)
                );
            }
            
            // Generate output filename
            let base_filename = format!("{}_{}.png", Path::new(input_path).file_stem().and_then(|s| s.to_str()).unwrap_or("output"), i + 1);
            
            // Determine output directory
            let output_dir = if let Some(ref custom_dir) = self.config.output_dir {
                custom_dir.clone()
            } else {
                // Default to the input file's directory
                Path::new(input_path)
                    .parent()
                    .and_then(|p| p.to_str())
                    .unwrap_or(".")
                    .to_string()
            };
            
            let output_filename = format!("{}/{}", output_dir, base_filename);
            
            cropped_img
                .save(&output_filename)
                .map_err(|e| format!("Failed to save cropped image: {}", e))?;
            output_files.push(output_filename);
        }

        if self.config.debug_enabled {
            self.save_debug_preview(&img, &detection_result, scaling_factor, input_path)?;
        }

        Ok(ProcessingResult {
            input_path: input_path.to_string(),
            output_files,
        })
    }

    pub fn process_directory(&self, dir_path: &str) -> Result<Vec<ProcessingResult>, String> {
        let path = Path::new(dir_path);
        if !path.is_dir() {
            return Err("Invalid directory path".to_string());
        }

        let mut results = Vec::new();

        for entry in std::fs::read_dir(path).map_err(|e| format!("Failed to read folder: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let file_path = entry.path();

            if file_path.is_file() {
                if let Some(file_name) = file_path.file_name().and_then(|n| n.to_str()) {
                    if file_name.starts_with(&self.config.prefix) {
                        match self.process_file(file_path.to_str().unwrap()) {
                            Ok(result) => results.push(result),
                            Err(e) => log_message!(LogLevel::Error, &e),
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    fn save_debug_preview(
        &self,
        img: &image::DynamicImage,
        detection_result: &crate::image_ops::DetectionResult,
        scaling_factor: f64,
        input_path: &str,
    ) -> Result<(), String> {
        use image::Rgb;
        
        let debug_info = detection_result.debug_info.as_ref()
            .ok_or("Debug info not available")?;
        
        // Determine output directory
        let output_dir = if let Some(ref custom_dir) = self.config.output_dir {
            custom_dir.clone()
        } else {
            Path::new(input_path)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or(".")
                .to_string()
        };
        
        // Convert grayscale intermediate images to RGB for visualization
        let blurred_rgb = gray_to_rgb(&debug_info.blurred);
        let edges_rgb = gray_to_rgb(&debug_info.edges);
        let dilated_rgb = gray_to_rgb(&debug_info.dilated);
        let closed_rgb = gray_to_rgb(&debug_info.closed);
        let cleaned_rgb = gray_to_rgb(&debug_info.cleaned);
        
        // Create final result image with accepted boxes (green) and rejected boxes (red)
        let mut result_img = img.clone().to_rgb8();
        
        // Draw rejected candidates in red
        let accepted_rects: Vec<_> = detection_result.photos.iter().map(|p| &p.rect).collect();
        for (rect, rectangularity) in &debug_info.all_contours {
            let is_accepted = accepted_rects.contains(&rect);
            if !is_accepted {
                let scaled_x = (rect.min().x as f64 / scaling_factor).round() as u32;
                let scaled_y = (rect.min().y as f64 / scaling_factor).round() as u32;
                let scaled_w = (rect.width() as f64 / scaling_factor).round() as u32;
                let scaled_h = (rect.height() as f64 / scaling_factor).round() as u32;
                
                // Draw red box for rejected
                draw_rect_with_label(
                    &mut result_img,
                    scaled_x,
                    scaled_y,
                    scaled_w,
                    scaled_h,
                    Rgb([255, 0, 0]),
                    &format!("X {:.2}", rectangularity)
                );
            }
        }
        
        // Draw accepted boxes in green (on top)
        for photo in &detection_result.photos {
            let rect = &photo.rect;
            let scaled_x = (rect.min().x as f64 / scaling_factor).round() as u32;
            let scaled_y = (rect.min().y as f64 / scaling_factor).round() as u32;
            let scaled_w = (rect.width() as f64 / scaling_factor).round() as u32;
            let scaled_h = (rect.height() as f64 / scaling_factor).round() as u32;
            
            // Find rectangularity for this rect
            let rectangularity = debug_info.all_contours
                .iter()
                .find(|(r, _)| r == rect)
                .map(|(_, s)| *s)
                .unwrap_or(0.0);
            
            // Draw green box for accepted (with rotation angle if present)
            let label = if photo.rotation_angle.abs() > 0.5 {
                format!("✓ {:.2} (∠{:.1}°)", rectangularity, photo.rotation_angle)
            } else {
                format!("✓ {:.2}", rectangularity)
            };
            
            draw_rect_with_label(
                &mut result_img,
                scaled_x,
                scaled_y,
                scaled_w,
                scaled_h,
                Rgb([0, 255, 0]),
                &label
            );
        }
        
        // Create a composite preview image showing all stages
        let preview = create_preview_grid(
            vec![
                ("1. Blurred", blurred_rgb),
                ("2. Canny Edges", edges_rgb),
                ("3. Dilated", dilated_rgb),
                ("4. Closed (Morphological)", closed_rgb),
                ("5. Final Edges", cleaned_rgb),
                ("6. Detected Candidates", result_img),
            ],
            2 // columns
        )?;
        
        let debug_filename = format!("{}_debug_preview.png", 
            Path::new(input_path).file_stem().and_then(|s| s.to_str()).unwrap_or("debug"));
        let output_path = format!("{}/{}", output_dir, debug_filename);
        
        preview
            .save(&output_path)
            .map_err(|e| format!("Failed to save debug preview: {}", e))?;
        
        log_message!(LogLevel::Info, &format!("Debug preview saved to: {}", output_path));
        
        Ok(())
    }
}

fn gray_to_rgb(gray: &image::GrayImage) -> image::RgbImage {
    let (width, height) = gray.dimensions();
    let mut rgb = image::RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let val = gray.get_pixel(x, y)[0];
            rgb.put_pixel(x, y, image::Rgb([val, val, val]));
        }
    }
    rgb
}

fn draw_rect_with_label(
    img: &mut image::RgbImage,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    color: image::Rgb<u8>,
    _label: &str, // Label parameter kept for future use but not rendered
) {
    // Draw hollow rectangle with thicker lines for better visibility
    let thickness = 2;
    for i in 0..thickness {
        let rect = imageproc::rect::Rect::at((x as i32) - i, (y as i32) - i)
            .of_size(w + (2 * i as u32), h + (2 * i as u32));
        imageproc::drawing::draw_hollow_rect_mut(img, rect, color);
    }
}

fn draw_label_box(
    img: &mut image::RgbImage,
    x: u32,
    y: u32,
    width: u32,
    label: &str,
) {
    // Draw a colored label bar with the text prominently displayed
    let label_height = 35u32;
    
    // Determine color based on step number (extract from label)
    let color = if label.starts_with("1") {
        image::Rgb([100, 149, 237]) // Cornflower blue
    } else if label.starts_with("2") {
        image::Rgb([60, 179, 113]) // Medium sea green
    } else if label.starts_with("3") {
        image::Rgb([255, 165, 0]) // Orange
    } else if label.starts_with("4") {
        image::Rgb([220, 20, 60]) // Crimson
    } else if label.starts_with("5") {
        image::Rgb([147, 112, 219]) // Medium purple
    } else if label.starts_with("6") {
        image::Rgb([255, 215, 0]) // Gold
    } else {
        image::Rgb([128, 128, 128]) // Gray
    };
    
    // Draw colored background bar
    imageproc::drawing::draw_filled_rect_mut(
        img,
        imageproc::rect::Rect::at(x as i32, y as i32).of_size(width, label_height),
        color,
    );
    
    // Draw black border
    for i in 0..2 {
        imageproc::drawing::draw_hollow_rect_mut(
            img,
            imageproc::rect::Rect::at(x as i32 + i, y as i32 + i)
                .of_size(width - 2 * i as u32, label_height - 2 * i as u32),
            image::Rgb([0, 0, 0]),
        );
    }
    
    // Draw large step number on the left
    let step_num_size = 20u32;
    let step_num_x = x + 5;
    let step_num_y = y + (label_height - step_num_size) / 2;
    
    if let Some(first_char) = label.chars().next() {
        if first_char.is_ascii_digit() {
            imageproc::drawing::draw_filled_rect_mut(
                img,
                imageproc::rect::Rect::at(step_num_x as i32, step_num_y as i32)
                    .of_size(step_num_size, step_num_size),
                image::Rgb([0, 0, 0]),
            );
            imageproc::drawing::draw_filled_rect_mut(
                img,
                imageproc::rect::Rect::at(step_num_x as i32 + 2, step_num_y as i32 + 2)
                    .of_size(step_num_size - 4, step_num_size - 4),
                image::Rgb([255, 255, 255]),
            );
        }
    }
    
    // Draw text label bars to represent the text (visual markers)
    let text_start_x = x + 35;
    let text_y = y + label_height / 2 - 3;
    let char_width = 8u32;
    
    for (idx, ch) in label.chars().skip(3).enumerate() { // Skip "1. " part
        if ch == ' ' {
            continue;
        }
        let char_x = text_start_x + idx as u32 * (char_width + 1);
        let height = if ch.is_uppercase() || ch.is_ascii_digit() { 6 } else { 4 };
        let y_offset = if ch.is_uppercase() || ch.is_ascii_digit() { 0 } else { 2 };
        
        imageproc::drawing::draw_filled_rect_mut(
            img,
            imageproc::rect::Rect::at(char_x as i32, (text_y + y_offset) as i32)
                .of_size(char_width - 1, height),
            image::Rgb([0, 0, 0]),
        );
    }
}

fn create_preview_grid(
    images: Vec<(&str, image::RgbImage)>,
    columns: usize,
) -> Result<image::RgbImage, String> {
    if images.is_empty() {
        return Err("No images to create preview".to_string());
    }
    
    // Find the maximum dimensions
    let max_width = images.iter().map(|(_, img)| img.width()).max().unwrap();
    let max_height = images.iter().map(|(_, img)| img.height()).max().unwrap();
    
    // Scale all images to fit in a reasonable preview size
    let target_width = 800u32;
    let scale = target_width as f64 / max_width as f64;
    let scaled_width = target_width;
    let scaled_height = (max_height as f64 * scale) as u32;
    
    let rows = (images.len() + columns - 1) / columns;
    let padding = 20u32;
    let label_height = 35u32; // Increased for better text visibility
    
    let grid_width = columns as u32 * (scaled_width + padding) + padding;
    let grid_height = rows as u32 * (scaled_height + label_height + padding) + padding;
    
    let mut grid = image::RgbImage::from_pixel(grid_width, grid_height, image::Rgb([40, 40, 40]));
    
    for (idx, (label, img)) in images.iter().enumerate() {
        let row = idx / columns;
        let col = idx % columns;
        
        let x = padding + col as u32 * (scaled_width + padding);
        let y = padding + row as u32 * (scaled_height + label_height + padding);
        
        // Resize image to fit
        let resized = image::imageops::resize(
            img,
            scaled_width,
            scaled_height,
            image::imageops::FilterType::Lanczos3,
        );
        
        // Draw colored label box
        draw_label_box(&mut grid, x, y, scaled_width, label);
        
        // Copy image
        image::imageops::overlay(&mut grid, &resized, x as i64, (y + label_height) as i64);
    }
    
    Ok(grid)
}
