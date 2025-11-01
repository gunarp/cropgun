#[macro_export]
macro_rules! log_message {
    ($level:expr, $message:expr) => {{
        let current_level =
            $crate::config::VERBOSITY_LEVEL.load(std::sync::atomic::Ordering::Relaxed);
        let current_level = match current_level {
            x if x == $crate::config::LogLevel::Error as usize => $crate::config::LogLevel::Error,
            x if x == $crate::config::LogLevel::Info as usize => $crate::config::LogLevel::Info,
            x if x == $crate::config::LogLevel::Verbose as usize => {
                $crate::config::LogLevel::Verbose
            }
            x if x == $crate::config::LogLevel::Debug as usize => $crate::config::LogLevel::Debug,
            _ => $crate::config::LogLevel::Info,
        };

        match ($level, current_level) {
            ($crate::config::LogLevel::Error, _) => println!("[ERROR] {}", $message),
            (
                $crate::config::LogLevel::Info,
                $crate::config::LogLevel::Info
                | $crate::config::LogLevel::Verbose
                | $crate::config::LogLevel::Debug,
            ) => {
                println!("[INFO] {}", $message)
            }
            (
                $crate::config::LogLevel::Verbose,
                $crate::config::LogLevel::Verbose | $crate::config::LogLevel::Debug,
            ) => {
                println!("[VERBOSE] {}", $message)
            }
            ($crate::config::LogLevel::Debug, $crate::config::LogLevel::Debug) => {
                println!("[DEBUG] {}", $message)
            }
            _ => (),
        }
    }};
}

#[macro_export]
macro_rules! start_timer {
    ($operation_name:expr) => {{
        log_message!(
            LogLevel::Verbose,
            &format!("========== STARTING: {} ==========", $operation_name)
        );
        std::time::Instant::now()
    }};
}

#[macro_export]
macro_rules! stop_timer {
    ($start_time:expr, $operation_name:expr) => {{
        let duration = $start_time.elapsed();
        log_message!(
            LogLevel::Verbose,
            &format!(
                "========== COMPLETED: {} in {:.2?} ==========",
                $operation_name, duration
            )
        );
    }};
}
