# Product Requirements Document (PRD)

## Project Name
Photo Digitizer CLI

## Objective
Develop a command-line application to digitize photographs by isolating individual images from a document scanner's output. The application will leverage computer vision (CV) techniques to detect and crop photographs from scanned documents.

## Key Features
1. **Input Handling**:
   - Accept a file path or folder path as input.
   - Support common image formats (e.g., JPEG, PNG, TIFF).
   - Allow an optional `dates_taken` argument:
     - If a list of dates is provided with the same number of elements as the identified images, apply the dates to the cropped images in order of left-to-right, top-to-bottom.
     - If a single date string is provided, apply the same date to all cropped images.

2. **Image Processing**:
   - Detect individual photographs within a scanned document.
   - Crop and save each detected photograph as a separate image file.
   - Apply the `dates_taken` metadata to the cropped images if provided.

3. **Output**:
   - Save processed images in a specified output directory.
   - Provide options for output image format and resolution.
   - Display performance statistics, including the time taken to process each image.

4. **Error Handling**:
   - Gracefully handle invalid file paths or unsupported formats.
   - Provide meaningful error messages for debugging.

## Technology Stack
- **Programming Language**: Rust
- **Key Libraries/Crates**:
  - `image` (for image manipulation and processing)
  - `cv` (from the `rust-cv` ecosystem for advanced computer vision techniques)
  - `rayon` (for parallel processing to improve performance)
  - `clap` (for building the CLI interface)
  - `log` and `env_logger` (for logging and debugging)
  - `instant` (for measuring and reporting performance statistics)

## Milestones
1. **MVP**:
   - Implement CLI with basic input/output functionality.
   - Integrate image loading and saving using the `image` crate.
   - Implement basic photograph detection and cropping using the `cv` crate from the `rust-cv` ecosystem.

2. **Performance Optimization**:
   - Add parallel processing for batch image processing using `rayon`.

3. **Advanced Features**:
   - Add support for custom output formats and resolutions.
   - Implement additional image enhancement techniques (e.g., color correction, noise reduction).

4. **Testing and Documentation**:
   - Write unit tests for core functionality.
   - Provide a comprehensive README with usage instructions.

## Success Metrics
- Accurate detection and cropping of individual photographs.
- Efficient processing of large batches of images.
- User-friendly CLI with clear documentation.
- **Performance Target**: Auto-cropping for a single image should complete in less than one second.

## Future Scope
- Develop a GUI version of the application.
- Add support for cloud-based processing and storage.
- Extend functionality to include metadata extraction and tagging.