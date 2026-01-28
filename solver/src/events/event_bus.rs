use crate::error::Result;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::broadcast;

use super::events::SolverEvent;

/// Event bus for pub/sub pattern
pub struct EventBus {
    sender: broadcast::Sender<SolverEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub async fn publish(&self, event: SolverEvent) -> Result<()> {
        let _ = self.sender.send(event.clone());
        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SolverEvent> {
        self.sender.subscribe()
    }

    pub fn start_heartbeat(&self) {
        let sender = self.sender.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

            loop {
                interval.tick().await;

                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis();

                let _ = sender.send(SolverEvent::Heartbeat(timestamp));
            }
        });
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
