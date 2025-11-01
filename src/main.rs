use clap::{ArgGroup, Parser};
use cropgun::{
    config::{set_verbosity_level, LogLevel, ProcessingConfig},
    log_message, DaemonWatcher, ImageProcessor,
};
use std::path::Path;
use std::time::Instant;
use cropgun::config::PreprocessingMode;

/// CLI tool to process images
#[derive(Parser)]
#[command(group(ArgGroup::new("verbosity").args(["verbose", "debug"]).multiple(false)))]
struct Cli {
    /// Path to the input PNG file or folder
    input: String,

    /// Enable verbose logging
    #[arg(long, action = clap::ArgAction::SetTrue)]
    verbose: bool,

    /// Enable debug logging (implies verbose)
    #[arg(long, action = clap::ArgAction::SetTrue)]
    debug: bool,

    /// Optional prefix for filtering files in a folder
    #[arg(long, default_value = "IMG_")]
    prefix: String,

    /// Enable daemon mode to watch a folder for new files
    #[arg(long, action = clap::ArgAction::SetTrue)]
    daemon: bool,

    /// Preprocessing mode:
    /// 0 = No preprocessing
    /// 1 = Network optimized preprocessing (shrink image)
    #[arg(long, default_value_t = 0)]
    preprocess_mode: u8,

    /// Optional output directory for saving processed images
    /// If not specified, images will be saved in the current directory
    #[arg(long)]
    output_dir: Option<String>,
}

fn determine_path_type(input: &str) -> Result<&str, &str> {
    let path = Path::new(input);
    if path.is_dir() {
        Ok("folder")
    } else if path.is_file() {
        Ok("file")
    } else {
        Err("Invalid path.")
    }
}

fn main() {
    let args = Cli::parse();
    set_verbosity_level(args.verbose, args.debug);

    // Create output directory if specified
    if let Some(ref output_dir) = args.output_dir {
        std::fs::create_dir_all(output_dir).map_err(|e| {
            log_message!(LogLevel::Error, &format!("Failed to create output directory: {}", e));
        }).ok();
    }

    match determine_path_type(&args.input) {
        Ok("file") => {
            log_message!(LogLevel::Info, "File path detected");
        }
        Ok("folder") => {
            log_message!(LogLevel::Info, "Folder path detected");
        }
        Err(e) => {
            log_message!(LogLevel::Error, e);
            return;
        }
        _ => unreachable!(),
    }

    if args.daemon {
        let config = ProcessingConfig {
            debug_enabled: args.debug,
            prefix: args.prefix.clone(),
            preprocess_mode: PreprocessingMode::from_u8(args.preprocess_mode).unwrap_or(PreprocessingMode::None),
            output_dir: args.output_dir.clone(),
        };

        let daemon = DaemonWatcher::new(config);
        if let Err(e) = daemon.run(&args.input) {
            log_message!(LogLevel::Error, &e);
        }
        return;
    }

    let start_time = Instant::now();
    let config = ProcessingConfig {
        debug_enabled: args.debug,
        prefix: args.prefix.clone(),
        preprocess_mode: PreprocessingMode::from_u8(args.preprocess_mode).unwrap_or(PreprocessingMode::None),
        output_dir: args.output_dir.clone(),
    };
    let processor = ImageProcessor::new(config);

    match determine_path_type(&args.input) {
        Ok("file") => match processor.process_file(&args.input) {
            Ok(result) => {
                log_message!(
                    LogLevel::Info,
                    &format!("Processed file: {}", result.input_path)
                );
                for output in result.output_files {
                    log_message!(LogLevel::Info, &format!("Output: {}", output));
                }
            }
            Err(e) => log_message!(LogLevel::Error, &e),
        },
        Ok("folder") => match processor.process_directory(&args.input) {
            Ok(results) => {
                log_message!(
                    LogLevel::Info,
                    &format!("Processed {} files", results.len())
                );
            }
            Err(e) => log_message!(LogLevel::Error, &e),
        },
        Err(e) => {
            log_message!(LogLevel::Error, e);
            return;
        }
        _ => unreachable!(),
    }

    // log the total duration
    let duration = start_time.elapsed();
    log_message!(
        LogLevel::Info,
        &format!("Total processing time: {:.2?}", duration)
    );
}
