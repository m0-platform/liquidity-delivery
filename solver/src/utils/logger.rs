/// Creates common key-value pairs for logger initialization.
/// This includes timestamp and other fields that may be added in the future.
#[macro_export]
macro_rules! common_logger_values {
    () => {
        slog::o!("timestamp" => slog::FnValue(|_| {
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        }))
    };
}
