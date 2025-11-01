use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LogLevel {
    Error,
    Info,
    Verbose,
    Debug,
}

pub static VERBOSITY_LEVEL: AtomicUsize = AtomicUsize::new(LogLevel::Info as usize);

pub fn set_verbosity_level(verbose: bool, debug: bool) {
    let level = if debug {
        LogLevel::Debug
    } else if verbose {
        LogLevel::Verbose
    } else {
        LogLevel::Info
    };

    VERBOSITY_LEVEL.store(level as usize, Ordering::Relaxed);
}

pub fn get_verbosity_level() -> LogLevel {
    match VERBOSITY_LEVEL.load(Ordering::Relaxed) {
        x if x == LogLevel::Error as usize => LogLevel::Error,
        x if x == LogLevel::Info as usize => LogLevel::Info,
        x if x == LogLevel::Verbose as usize => LogLevel::Verbose,
        x if x == LogLevel::Debug as usize => LogLevel::Debug,
        _ => LogLevel::Info,
    }
}

#[derive(Clone, PartialEq)]
pub enum PreprocessingMode {
    None,
    NetworkOptimized,
}

impl PreprocessingMode {
    pub fn from_u8(value: u8) -> Result<Self, String> {
        match value {
            0 => Ok(PreprocessingMode::None),
            1 => Ok(PreprocessingMode::NetworkOptimized),
            _ => Err(format!("Invalid preprocessing mode: {}", value)),
        }
    }
}

#[derive(Clone)]
pub struct ProcessingConfig {
    pub debug_enabled: bool,
    pub prefix: String,
    pub preprocess_mode: PreprocessingMode,
    pub output_dir: Option<String>,
}

pub struct ProcessingResult {
    pub input_path: String,
    pub output_files: Vec<String>,
}
