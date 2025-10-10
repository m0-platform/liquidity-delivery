use crate::error::Result;
use std::fmt;
use std::sync::Arc;
use tokio::sync::broadcast;

use super::events::SolverEvent;

/// Event bus for pub/sub pattern
pub struct EventBus {
    sender: broadcast::Sender<Arc<SolverEvent>>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish an event to all subscribers
    pub async fn publish(&self, event: Arc<SolverEvent>) -> Result<()> {
        let _ = self.sender.send(event.clone());
        Ok(())
    }

    /// Subscribe to events (returns a receiver)
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<SolverEvent>> {
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

impl fmt::Debug for EventBus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventBus").finish()
    }
}
