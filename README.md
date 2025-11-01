# Cropgun

Cropgun is a command-line image processing tool written in Rust that automatically detects and crops rectangular objects (gun shapes, targets, etc.) from images. It uses edge detection and contour analysis to identify regions of interest and extract them as separate image files.

## Features

- **Automatic Object Detection**: Uses Gaussian blur, edge detection, and contour analysis to identify rectangular objects in images.
- **Batch Processing**: Process entire directories of images with configurable file prefix filtering.
- **Daemon Mode**: Watch a folder for new images and automatically process them as they arrive.
- **Preprocessing Options**: Optional image resizing to optimize performance and memory usage.
- **Debug Mode**: Generate annotated debug images showing detected bounding boxes.
- **Flexible Output**: Save processed images to a custom output directory.
- **Logging**: Multiple verbosity levels for monitoring and debugging.

## Installation

### Prerequisites

- Rust 1.70 or later ([Install Rust](https://www.rust-lang.org/tools/install))

### Build from Source

```bash
git clone <repository-url>
cd cropgun
cargo build --release
```

The compiled binary will be located at `target/release/cropgun.exe` (Windows) or `target/release/cropgun` (Linux/macOS).

## Usage

### Basic Syntax

```bash
cropgun <INPUT_PATH> [OPTIONS]
```

### Arguments

- `<INPUT_PATH>`: Path to a PNG image file or a directory containing images

### Options

#### Logging Options
- `--verbose`: Enable verbose logging output
- `--debug`: Enable debug logging (implies verbose mode, shows detailed processing information)

#### Preprocessing Options
- `--preprocess-mode <VALUE>`: Set image preprocessing mode (default: `0`)
  - `0`: No preprocessing (process images at full resolution)
  - `1`: Network optimized preprocessing (resizes images to standard height of 1000px, improving speed and memory usage)

#### File Processing Options
- `--prefix <PREFIX>`: Filter files by prefix when processing directories (default: `IMG_`)
  - Only files starting with this prefix will be processed
  - Example: `--prefix "photo_"` will only process files like `photo_001.png`

#### Output Options
- `--output-dir <PATH>`: Directory where processed images will be saved
  - The directory will be created automatically if it doesn't exist
  - If not specified, images are saved in the current directory

#### Daemon Mode
- `--daemon`: Enable daemon mode to continuously monitor a folder for new images
  - Useful for automated batch processing workflows
  - The tool will watch for new files matching the specified prefix and process them automatically

## Examples

### Process a single image
```bash
cropgun image.png
```

### Process with verbose logging
```bash
cropgun image.png --verbose
```

### Process with debug logging and save to output folder
```bash
cropgun image.png --debug --output-dir ./results
```

### Process with preprocessing to improve performance
```bash
cropgun image.png --preprocess-mode 1
```

### Batch process all images in a directory
```bash
cropgun ./images --prefix IMG_
```

### Batch process with preprocessing and custom output directory
```bash
cropgun ./images --prefix IMG_ --preprocess-mode 1 --output-dir ./processed
```

### Watch a folder for new images and process them automatically
```bash
cropgun ./watch_folder --daemon --prefix IMG_ --output-dir ./results
```

### Full example with all options
```bash
cropgun ./input_images --prefix IMG_ --preprocess-mode 1 --output-dir ./output --verbose
```

## Output

For each image processed, the tool generates:

1. **Cropped Images**: One or more PNG files containing the detected objects
   - Naming convention: `{original_filename}_{index}.png`
   - Example: `image_1.png`, `image_2.png`, etc.

2. **Debug Images** (when `--debug` is enabled):
   - Naming convention: `{original_filename}_debug.png`
   - Shows the original image with red bounding boxes around detected objects

3. **Logs**: Processing information printed to console (verbosity level controlled by `--verbose`/`--debug`)

## Processing Pipeline

The tool processes images through the following steps:

1. **Image Loading**: Read the PNG image into memory
2. **Preprocessing** (optional):
   - If `--preprocess-mode 1` is specified: Resize image to standard height (1000px) while maintaining aspect ratio
   - Helps improve processing speed and reduce memory usage
3. **Grayscale Conversion**: Convert the image to grayscale for edge detection
4. **Edge Detection**:
   - Apply Gaussian blur to smooth the image
   - Invert the image
   - Detect contours using threshold-based detection
5. **Bounding Box Filtering**:
   - Filter contours by minimum area (5% of image area)
   - Generate bounding rectangles for qualifying objects
6. **Image Cropping**: Extract and save each detected object as a separate image
7. **Scaling Compensation**: Automatically adjust crop coordinates if preprocessing was applied

## Performance Tips

- **Use Preprocessing**: For large images, use `--preprocess-mode 1` to significantly improve processing speed
- **Filter by Prefix**: When batch processing, use appropriate prefix filters to avoid processing unwanted files
- **Daemon Mode**: For continuous workflows, use daemon mode to reduce startup overhead
- **Debug Mode**: Use debug images to verify detection accuracy before processing large batches

## Troubleshooting

### No objects detected
- Check image quality and lighting conditions
- Verify that objects have clear edges and contrast
- Enable `--debug` to visualize the detection process
- Consider adjusting image preprocessing

### Incorrect crop boundaries
- Enable `--debug` to view the detected bounding boxes
- If using preprocessing, verify that the scaling is correct
- Check image resolution and contrast

### Performance issues
- Enable `--preprocess-mode 1` to reduce image size
- Process smaller batches of images
- Monitor available system memory

## Logging Levels

- **Info** (default): Basic operation information
- **Verbose**: Detailed processing steps and timing information
- **Debug**: Comprehensive debugging information including contour details and bounding boxes
