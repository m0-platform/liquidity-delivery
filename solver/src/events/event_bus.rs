use crate::error::Result;
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

    /// Publish an event to all subscribers
    pub async fn publish(&self, event: SolverEvent) -> Result<()> {
        let _ = self.sender.send(event.clone());
        Ok(())
    }

    /// Subscribe to events (returns a receiver)
    pub fn subscribe(&self) -> broadcast::Receiver<SolverEvent> {
        self.sender.subscribe()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
