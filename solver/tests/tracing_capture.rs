use std::fmt::Debug;
use std::io::{stderr, Write};
use std::sync::{Arc, Mutex, RwLock};
use tracing::level_filters::LevelFilter;
use tracing::Subscriber;
use tracing_subscriber::{fmt::MakeWriter, layer::SubscriberExt, Layer};

/// A guard that manages a test-local tracing subscriber
/// When dropped, it will flush logs if the test is panicking
pub struct TestTracingGuard {
    capture: CaptureLayer,
    _guard: tracing::subscriber::DefaultGuard,
}

impl TestTracingGuard {
    /// Check if any captured log contains the given substring
    pub fn contains(&self, substring: &str) -> bool {
        self.capture.contains(substring)
    }
}

impl Drop for TestTracingGuard {
    fn drop(&mut self) {
        // Check if we're panicking (test failed)
        if std::thread::panicking() {
            self.capture.flush();
        }
    }
}

/// A layer that captures log messages for testing
#[derive(Clone)]
struct CaptureLayer {
    logs: Arc<RwLock<Vec<String>>>,
    writer: BufferedWriter,
}

impl CaptureLayer {
    fn new(writer: BufferedWriter) -> Self {
        Self {
            logs: Arc::new(RwLock::new(Vec::new())),
            writer,
        }
    }

    /// Check if any captured log contains the given substring
    fn contains(&self, substring: &str) -> bool {
        self.logs
            .read()
            .unwrap()
            .iter()
            .any(|log| log.contains(substring))
    }

    /// Flush buffered logs to stderr
    fn flush(&self) {
        self.writer.flush_to_stderr();
    }
}

impl<S> Layer<S> for CaptureLayer
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        struct SimpleVisitor {
            fields: Vec<(String, String)>,
        }

        impl tracing::field::Visit for SimpleVisitor {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn Debug) {
                self.fields
                    .push((field.name().to_string(), format!("{:?}", value)));
            }
        }

        let mut visitor = SimpleVisitor { fields: Vec::new() };
        event.record(&mut visitor);

        // Build message from collected fields
        let message = visitor
            .fields
            .iter()
            .map(|(name, value)| format!("{}={}", name, value))
            .collect::<Vec<_>>()
            .join(" ");

        if let Ok(mut logs) = self.logs.write() {
            logs.push(message);
        }
    }
}

/// Initialize the tracing subscriber for a single test and return a guard
/// Each call creates a new isolated subscriber for the test that is automatically
/// cleaned up when the guard is dropped
pub fn init_test_tracing() -> TestTracingGuard {
    let writer = BufferedWriter::new();
    let capture_layer = CaptureLayer::new(writer.clone());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_writer(writer)
        .with_filter(LevelFilter::INFO);

    // Create a new subscriber for this test with a guard
    let subscriber = tracing_subscriber::registry()
        .with(capture_layer.clone())
        .with(fmt_layer);

    let guard = tracing::subscriber::set_default(subscriber);

    TestTracingGuard {
        capture: capture_layer,
        _guard: guard,
    }
}

/// A writer that buffers output and can be flushed on demand
#[derive(Clone)]
struct BufferedWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl BufferedWriter {
    fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn flush_to_stderr(&self) {
        if let Ok(buffer) = self.buffer.lock() {
            if !buffer.is_empty() {
                let _ = stderr().write_all(&buffer);
                let _ = stderr().flush();
            }
        }
    }
}

impl Write for BufferedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.write(buf)
        } else {
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for BufferedWriter {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}
