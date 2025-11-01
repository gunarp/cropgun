use crate::config::{ProcessingConfig, LogLevel};
use crate::log_message;
use crate::image_processor::ImageProcessor;
use std::path::Path;
use std::path::PathBuf;
use std::collections::HashSet;

pub struct DaemonWatcher {
    config: ProcessingConfig,
    processor: ImageProcessor,
}

impl DaemonWatcher {
    pub fn new(config: ProcessingConfig) -> Self {
        let processor = ImageProcessor::new(config.clone());
        DaemonWatcher { config, processor }
    }

    pub fn run(&self, folder_path: &str) -> Result<(), String> {
        let path = Path::new(folder_path);
        if !path.is_dir() {
            return Err("Daemon mode requires a valid folder path".to_string());
        }

        log_message!(LogLevel::Info, "Daemon mode enabled. Watching folder for new files...");

        let mut processed_files: HashSet<PathBuf> = std::fs::read_dir(path)
            .map_err(|e| format!("Failed to read folder: {}", e))?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect();

        let (tx, rx) = std::sync::mpsc::channel();
        let prefix = self.config.prefix.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if let notify::EventKind::Create(_) = event.kind {
                    for path in event.paths {
                        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                            if file_name.starts_with(&prefix) {
                                tx.send(path).expect("Failed to send path through channel");
                            }
                        }
                    }
                }
            }
        }).map_err(|e| format!("Failed to create watcher: {}", e))?;

        notify::Watcher::watch(&mut watcher, path, notify::RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch folder: {}", e))?;

        log_message!(
            LogLevel::Info,
            &format!(
                "Pre-populated processed files with {} existing files.",
                processed_files.len()
            )
        );

        for path in rx {
            if processed_files.contains(&path) {
                log_message!(LogLevel::Verbose, &format!("File {:?} already processed. Skipping.", path));
                continue;
            }

            log_message!(LogLevel::Info, &format!("New file detected: {:?}", path));

            let mut attempts = 0;
            loop {
                match image::open(&path) {
                    Ok(_) => break,
                    Err(e) if attempts < 10 => {
                        attempts += 1;
                        log_message!(
                            LogLevel::Debug,
                            &format!(
                                "Attempt {} to open file {:?} failed: {}. Retrying...",
                                attempts, path, e
                            )
                        );
                        std::thread::sleep(std::time::Duration::from_millis(2u64.pow(attempts) * 100));
                    }
                    Err(e) => {
                        log_message!(
                            LogLevel::Error,
                            &format!("Failed to open file {:?} after 10 attempts: {}", path, e)
                        );
                        continue;
                    }
                }
            }

            match self.processor.process_file(path.to_str().unwrap()) {
                Ok(result) => {
                    processed_files.extend(result.output_files.into_iter().map(PathBuf::from));
                }
                Err(e) => {
                    log_message!(LogLevel::Error, &e);
                }
            }
        }

        Ok(())
    }
}
