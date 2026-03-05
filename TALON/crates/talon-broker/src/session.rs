use std::collections::HashMap;
use std::sync::Arc;

use talon_types::broker::*;
use talon_types::error::BrokerError;
use talon_types::order::{OrderId, OrderIntent, OrderModify};
use talon_types::position::Position;

use crate::traits::{BrokerCommands, BrokerStreams};

// ---------------------------------------------------------------------------
// BrokerSessionManager — wraps sync calls in spawn_blocking (S6.2)
// ---------------------------------------------------------------------------

pub struct BrokerSessionManager {
    sessions: HashMap<BrokerId, BrokerSession>,
}

struct BrokerSession {
    commands: Arc<dyn BrokerCommands>,
    streams: Arc<dyn BrokerStreams>,
}

impl BrokerSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        id: BrokerId,
        commands: Arc<dyn BrokerCommands>,
        streams: Arc<dyn BrokerStreams>,
    ) {
        self.sessions.insert(id, BrokerSession { commands, streams });
    }

    pub fn commands(&self, id: &BrokerId) -> Option<Arc<dyn BrokerCommands>> {
        self.sessions.get(id).map(|s| Arc::clone(&s.commands))
    }

    pub fn streams(&self, id: &BrokerId) -> Option<Arc<dyn BrokerStreams>> {
        self.sessions.get(id).map(|s| Arc::clone(&s.streams))
    }

    /// Submit order via spawn_blocking (S6.2 pattern).
    pub async fn submit(
        &self,
        broker: &BrokerId,
        order: &OrderIntent,
    ) -> Result<OrderAck, BrokerError> {
        let commands = self
            .commands(broker)
            .ok_or_else(|| BrokerError::ConnectionLost(format!("no session for {broker}")))?;
        let order = order.clone();
        tokio::task::spawn_blocking(move || commands.submit_order(&order))
            .await
            .map_err(|e| BrokerError::RuntimePanic(e.to_string()))?
    }

    pub async fn cancel(
        &self,
        broker: &BrokerId,
        order_id: &OrderId,
    ) -> Result<CancelAck, BrokerError> {
        let commands = self
            .commands(broker)
            .ok_or_else(|| BrokerError::ConnectionLost(format!("no session for {broker}")))?;
        let oid = *order_id;
        tokio::task::spawn_blocking(move || commands.cancel_order(&oid))
            .await
            .map_err(|e| BrokerError::RuntimePanic(e.to_string()))?
    }

    pub async fn modify(
        &self,
        broker: &BrokerId,
        order_id: &OrderId,
        modification: &OrderModify,
    ) -> Result<ModifyAck, BrokerError> {
        let commands = self
            .commands(broker)
            .ok_or_else(|| BrokerError::ConnectionLost(format!("no session for {broker}")))?;
        let oid = *order_id;
        let m = modification.clone();
        tokio::task::spawn_blocking(move || commands.modify_order(&oid, &m))
            .await
            .map_err(|e| BrokerError::RuntimePanic(e.to_string()))?
    }

    pub async fn positions(
        &self,
        broker: &BrokerId,
    ) -> Result<Vec<Position>, BrokerError> {
        let commands = self
            .commands(broker)
            .ok_or_else(|| BrokerError::ConnectionLost(format!("no session for {broker}")))?;
        tokio::task::spawn_blocking(move || commands.positions())
            .await
            .map_err(|e| BrokerError::RuntimePanic(e.to_string()))?
    }

    pub async fn account_snapshot(
        &self,
        broker: &BrokerId,
    ) -> Result<AccountSnapshot, BrokerError> {
        let commands = self
            .commands(broker)
            .ok_or_else(|| BrokerError::ConnectionLost(format!("no session for {broker}")))?;
        tokio::task::spawn_blocking(move || commands.account_snapshot())
            .await
            .map_err(|e| BrokerError::RuntimePanic(e.to_string()))?
    }
}

impl Default for BrokerSessionManager {
    fn default() -> Self {
        Self::new()
    }
}
