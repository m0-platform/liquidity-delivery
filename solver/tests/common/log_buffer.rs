use slog::{Drain, OwnedKVList, Record, KV};
use std::{
    io::Write,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct LogBuffer {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn to_string(&self) -> String {
        let buffer = self.buffer.lock().unwrap();
        String::from_utf8_lossy(&buffer).to_string()
    }
}

impl Drain for LogBuffer {
    type Ok = ();
    type Err = std::io::Error;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        let mut buffer = self.buffer.lock().unwrap();

        // Write the log message in a simple key=value format
        write!(buffer, "{}", record.msg())?;

        // Write structured fields
        let mut serializer = KeyValueSerializer {
            buffer: &mut *buffer,
        };

        // Serialize record values
        let _ = values.serialize(record, &mut serializer);
        let _ = record.kv().serialize(record, &mut serializer);

        writeln!(buffer)?;
        Ok(())
    }
}

/// Helper to serialize key-value pairs from slog
struct KeyValueSerializer<'a> {
    buffer: &'a mut Vec<u8>,
}

impl<'a> slog::Serializer for KeyValueSerializer<'a> {
    fn emit_arguments(&mut self, key: slog::Key, val: &std::fmt::Arguments) -> slog::Result {
        write!(self.buffer, " {}={}", key, val)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok(())
    }
}
