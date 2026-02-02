use serde::Serialize;
use serde_json;
use slog::{Drain, Key, OwnedKVList, Record, KV};
use std::collections::HashMap;
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

/// A log entry to be sent to Loki
#[derive(Debug, Clone)]
struct LogEntry {
    timestamp_ns: String,
    message: String,
    level: String,
    labels: HashMap<String, String>,
}

/// Loki push API request format
#[derive(Serialize)]
struct LokiPushRequest {
    streams: Vec<LokiStream>,
}

#[derive(Serialize)]
struct LokiStream {
    stream: HashMap<String, String>,
    values: Vec<[String; 2]>,
}

pub struct LokiDrain {
    sender: Mutex<Sender<LogEntry>>,
}

impl LokiDrain {
    /// Create a new LokiDrain that sends logs to the specified Loki URL
    pub fn new(loki_url: String, labels: HashMap<String, String>) -> Self {
        let (sender, receiver) = mpsc::channel::<LogEntry>();

        // Spawn background task to batch and send logs to Loki
        let push_url = format!("{}/loki/api/v1/push", loki_url);
        let default_labels = labels.clone();

        std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            let mut batch: Vec<LogEntry> = Vec::new();
            let batch_size = 100;
            let flush_interval = std::time::Duration::from_secs(1);
            let mut last_flush = std::time::Instant::now();

            loop {
                // Try to receive with timeout to allow periodic flushing
                match receiver.recv_timeout(flush_interval) {
                    Ok(entry) => {
                        batch.push(entry);

                        // Flush if batch is full
                        if batch.len() >= batch_size {
                            flush_batch(&client, &push_url, &mut batch, &default_labels);
                            last_flush = std::time::Instant::now();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        // Flush on timeout if we have entries
                        if !batch.is_empty() && last_flush.elapsed() >= flush_interval {
                            flush_batch(&client, &push_url, &mut batch, &default_labels);
                            last_flush = std::time::Instant::now();
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        // Channel closed, flush remaining and exit
                        if !batch.is_empty() {
                            flush_batch(&client, &push_url, &mut batch, &default_labels);
                        }
                        break;
                    }
                }
            }
        });

        LokiDrain {
            sender: Mutex::new(sender),
        }
    }
}

fn flush_batch(
    client: &reqwest::blocking::Client,
    push_url: &str,
    batch: &mut Vec<LogEntry>,
    default_labels: &HashMap<String, String>,
) {
    if batch.is_empty() {
        return;
    }

    // Group entries by their labels
    let mut streams_map: HashMap<String, Vec<[String; 2]>> = HashMap::new();

    for entry in batch.drain(..) {
        let mut labels = default_labels.clone();
        labels.insert("level".to_string(), entry.level);
        for (k, v) in entry.labels {
            labels.insert(k, v);
        }

        // Create a stable key for grouping
        let mut label_pairs: Vec<_> = labels.iter().collect();
        label_pairs.sort_by_key(|(k, _)| *k);
        let key = label_pairs
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(",");

        streams_map
            .entry(key)
            .or_default()
            .push([entry.timestamp_ns, entry.message]);
    }

    // Build the request
    let streams: Vec<LokiStream> = streams_map
        .into_iter()
        .map(|(key, values)| {
            let labels: HashMap<String, String> = key
                .split(',')
                .filter_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    Some((parts.next()?.to_string(), parts.next()?.to_string()))
                })
                .collect();
            LokiStream {
                stream: labels,
                values,
            }
        })
        .collect();

    let request = LokiPushRequest { streams };

    // Send to Loki
    match client
        .post(push_url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
    {
        Ok(response) => {
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().unwrap_or_default();
                eprintln!("Loki rejected logs: status={}, body={}", status, body);
            }
        }
        Err(e) => {
            eprintln!("Failed to send logs to Loki: {}", e);
        }
    }
}

/// Helper struct to serialize key-value pairs from slog records
struct KVSerializer {
    values: HashMap<String, String>,
}

impl KVSerializer {
    fn new() -> Self {
        KVSerializer {
            values: HashMap::new(),
        }
    }
}

impl slog::Serializer for KVSerializer {
    fn emit_arguments(&mut self, key: Key, val: &std::fmt::Arguments) -> slog::Result {
        self.values.insert(key.to_string(), format!("{}", val));
        Ok(())
    }
}

impl Drain for LokiDrain {
    type Ok = ();
    type Err = slog::Never;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<Self::Ok, Self::Err> {
        // Get timestamp in nanoseconds
        let timestamp_ns = format!("{}", chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));

        // Collect key-value pairs
        let mut serializer = KVSerializer::new();
        let _ = values.serialize(record, &mut serializer);
        let _ = record.kv().serialize(record, &mut serializer);

        // Build message as JSON
        let mut json_map: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
        json_map.insert("msg".to_string(), serde_json::Value::String(format!("{}", record.msg())));
        json_map.insert("level".to_string(), serde_json::Value::String(record.level().as_str().to_string()));

        for (k, v) in serializer.values.iter() {
            if k != "timestamp" && k != "environment" && k != "component" {
                json_map.insert(k.clone(), serde_json::Value::String(v.clone()));
            }
        }

        let message = serde_json::to_string(&json_map).unwrap_or_else(|_| format!("{}", record.msg()));

        let entry = LogEntry {
            timestamp_ns,
            message,
            level: record.level().as_str().to_string(),
            labels: serializer
                .values
                .into_iter()
                .filter(|(k, _)| k == "component")
                .collect(),
        };

        // Send to background thread (non-blocking)
        if let Ok(sender) = self.sender.lock() {
            let _ = sender.send(entry);
        }

        Ok(())
    }
}
