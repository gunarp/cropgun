use crate::config::{LogLevel, ProcessingConfig, ProcessingResult};
use crate::image_ops::{detect_edges, process_image};
use crate::log_message;
use std::path::Path;

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

        let bounding_boxes = detect_edges(&processed_img);
        let mut output_files = Vec::new();

        for (i, rect) in bounding_boxes.iter().enumerate() {
            let scaled_x = (rect.min().x as f64 / scaling_factor).floor() as u32;
            let scaled_y = (rect.min().y as f64 / scaling_factor).floor() as u32;
            let scaled_w = (rect.width() as f64 / scaling_factor).ceil() as u32;
            let scaled_h = (rect.height() as f64 / scaling_factor).ceil() as u32;

            if scaled_x + scaled_w > img.width() || scaled_y + scaled_h > img.height() {
                return Err("Crop rectangle exceeds image bounds".to_string());
            }

            let cropped_img = img.crop_imm(scaled_x, scaled_y, scaled_w, scaled_h);
            
            // Generate output filename
            let base_filename = format!("{}_{}.png", Path::new(input_path).file_stem().and_then(|s| s.to_str()).unwrap_or("output"), i + 1);
            
            // Determine output path
            let output_filename = if let Some(ref output_dir) = self.config.output_dir {
                format!("{}/{}", output_dir, base_filename)
            } else {
                base_filename
            };
            
            cropped_img
                .save(&output_filename)
                .map_err(|e| format!("Failed to save cropped image: {}", e))?;
            output_files.push(output_filename);
        }

        if self.config.debug_enabled {
            self.save_debug_image(&img, &bounding_boxes, scaling_factor, input_path)?;
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

    fn save_debug_image(
        &self,
        img: &image::DynamicImage,
        bounding_boxes: &[geo::Rect<u32>],
        scaling_factor: f64,
        input_path: &str,
    ) -> Result<(), String> {
        let mut debug_img = img.clone().to_rgb8();

        for rect in bounding_boxes.iter() {
            let scaled_x = (rect.min().x as f64 / scaling_factor).round() as u32;
            let scaled_y = (rect.min().y as f64 / scaling_factor).round() as u32;
            let scaled_w = (rect.width() as f64 / scaling_factor).round() as u32;
            let scaled_h = (rect.height() as f64 / scaling_factor).round() as u32;

            imageproc::drawing::draw_hollow_rect_mut(
                &mut debug_img,
                imageproc::rect::Rect::at(scaled_x as i32, scaled_y as i32)
                    .of_size(scaled_w, scaled_h),
                image::Rgb([255, 0, 0]),
            );
        }

        let debug_filename = format!("{}_debug.png", Path::new(input_path).file_stem().and_then(|s| s.to_str()).unwrap_or("debug"));
        let output_path = if let Some(ref output_dir) = self.config.output_dir {
            format!("{}/{}", output_dir, debug_filename)
        } else {
            debug_filename
        };

        debug_img
            .save(&output_path)
            .map_err(|e| format!("Failed to save debug image: {}", e))?;

        Ok(())
    }
}
