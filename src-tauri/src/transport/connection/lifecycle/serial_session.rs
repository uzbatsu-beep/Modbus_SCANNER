//! Manager-owned exclusive serial session for parameter discovery.
//! Reserve the physical port before retiring an idle worker, and retain the
//! reservation through every configuration and all asynchronous cleanup.
use super::*;

#[derive(Debug, Clone)]
pub struct SerialSessionSnapshot {
    pub resource_key: String,
    pub connection_id: Option<String>,
    pub serial_configuration: Option<SerialPortConfig>,
    pub serial_mode: Option<SerialMode>,
    pub configuration_changes: u64,
    pub lease_count: usize,
    pub queue_depth: usize,
    pub active_requests: usize,
    pub reconnect_count: u64,
    pub recovery_remaining_ms: u64,
    pub exclusive_owner: String,
}

pub struct ExclusiveSerialSession {
    manager: Arc<ConnectionManager>,
    resource: String,
    reservation: Arc<SerialLease>,
    runtime: tokio::runtime::Handle,
    entry: Option<Arc<Entry>>,
    scope: Arc<OperationScope>,
    owner: String,
    configuration_changes: u64,
}

impl ConnectionManager {
    /// Refuse active leases, queued/active transactions, pinned entries and
    /// other exclusive owners. A quiescent automatic cache entry can be
    /// retired without waiting for its idle timeout.
    pub async fn exclusive_serial(
        self: &Arc<Self>, port: &str, owner: &str,
    ) -> TransportResult<ExclusiveSerialSession> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| invalid("exclusive serial session requires Tokio runtime"))?;
        if owner.trim().is_empty() { return Err(invalid("exclusive owner required")); }
        let resource = serial_key(port)?;
        let (reservation, retired) = {
            // Lock order matches register/acquire/lease_serials.
            let entries = self.entries.lock();
            let mut exclusive = self.exclusive.lock();
            if exclusive.contains(&resource) {
                return Err(invalid("physical serial resource already owned"));
            }
            let retired = entries.values().find(|e| e.resource == resource).cloned();
            if let Some(entry) = &retired {
                let lease = entry.lifecycle.lock();
                if !lease.automatic || lease.users != 0
                    || entry.slots.available_permits() != QUEUE_CAPACITY
                    || entry.state.lock().active_requests != 0
                {
                    return Err(invalid("physical serial resource already owned"));
                }
            }
            // The exclusive reservation spans removal of the old generation.
            exclusive.insert(resource.clone());
            if let Some(entry) = &retired { entry.stop.send_replace(true); }
            let reservation = Arc::new(SerialLease {
                exclusive: self.exclusive.clone(),
                resources: HashSet::from([resource.clone()]),
            });
            (reservation, retired)
        };
        if let Some(entry) = retired {
            // If the acquisition future is dropped, the join task keeps the
            // reservation until the old worker actually exits.
            let manager = self.clone();
            let held = reservation.clone();
            runtime.spawn(async move {
                let result = manager.disconnect_entry(entry).await;
                drop(held);
                result
            }).await.map_err(|e| TransportError::Other(e.to_string()))??;
        }
        Ok(ExclusiveSerialSession {
            manager: self.clone(), resource, reservation, runtime,
            entry: None, scope: Arc::new(OperationScope::default()),
            owner: owner.to_owned(), configuration_changes: 0,
        })
    }
}

impl ExclusiveSerialSession {
    /// Change settings only after joining the old worker. Reusing identical
    /// settings (including Unit ID changes at request level) keeps the worker.
    /// The new worker inherits the resource recovery deadline before opening.
    pub async fn configure(&mut self, mut definition: ConnectionDefinition) -> TransportResult<()> {
        if self.scope.is_cancelled() { return Err(TransportError::Cancelled); }
        if !matches!(&definition.transport, TransportConfig::Serial { .. })
            || definition.resource_key()? != self.resource
        {
            return Err(invalid("configuration must target reserved serial resource"));
        }
        if let Some(entry) = &self.entry {
            if !*entry.stop.borrow() && compatible(&entry.definition, &definition) {
                return Ok(());
            }
        }
        self.retire_current().await?;
        if self.scope.is_cancelled() { return Err(TransportError::Cancelled); }
        definition.id = format!("exclusive:{:032x}", rand::random::<u128>());
        let mut entries = self.manager.entries.lock();
        if entries.contains_key(&definition.id)
            || entries.values().any(|e| e.resource == self.resource)
        {
            return Err(invalid("physical serial resource already owned"));
        }
        // An automatic entry with one owner cannot be reaped or addressed
        // through the ID-only transaction API. The exclusive guard blocks acquire.
        let entry = self.manager.insert_entry(
            &mut entries, definition, self.resource.clone(),
            LeaseState::automatic(), &self.runtime,
        );
        self.entry = Some(entry);
        self.configuration_changes = self.configuration_changes.saturating_add(1);
        Ok(())
    }

    async fn retire_current(&mut self) -> TransportResult<()> {
        if let Some(entry) = self.entry.clone() {
            // Keep entry in self until join completes, so Drop can finish a
            // cancelled reconfiguration safely.
            self.manager.disconnect_entry(entry).await?;
            self.entry = None;
        }
        Ok(())
    }

    pub async fn transact(
        &self, request: TransactionRequest, validator: Option<ResponseValidator>,
    ) -> TransportResult<TransactionOutcome> {
        if self.scope.is_cancelled() { return Err(TransportError::Cancelled); }
        let entry = self.entry.clone().ok_or_else(|| invalid("serial session not configured"))?;
        self.manager.transact_entry(entry, request, self.scope.clone(), validator).await
    }

    pub fn snapshot(&self) -> SerialSessionSnapshot {
        let (config, mode) = self.entry.as_ref().map(|entry| {
            match &entry.definition.transport {
                TransportConfig::Serial { config, mode } => (Some(config.clone()), Some(*mode)),
                _ => (None, None),
            }
        }).unwrap_or((None,None));
        let remaining = self.manager.recovery.deadline(&self.resource)
            .map(|until| until.saturating_duration_since(tokio::time::Instant::now()))
            .unwrap_or_default();
        let (queue_depth, active_requests, reconnect_count) = self.entry.as_ref().map(|e|{
            let state=e.state.lock();
            (QUEUE_CAPACITY-e.slots.available_permits(),state.active_requests,state.reconnect_count)
        }).unwrap_or_default();
        SerialSessionSnapshot {
            resource_key: self.resource.clone(),
            connection_id: self.entry.as_ref().map(|e| e.definition.id.clone()),
            serial_configuration: config, serial_mode: mode,
            configuration_changes: self.configuration_changes,
            lease_count: 1, queue_depth, active_requests, reconnect_count,
            recovery_remaining_ms: remaining.as_millis().min(u64::MAX as u128) as u64,
            exclusive_owner: self.owner.clone(),
        }
    }

    /// Cancel only this session. Active I/O finishes within the worker bound.
    /// No further configuration or transaction is accepted after Stop.
    pub async fn stop(&self) { self.scope.stop().await; }

    pub async fn close(mut self) -> TransportResult<()> {
        self.stop().await;
        self.retire_current().await
    }
}

impl Drop for ExclusiveSerialSession {
    fn drop(&mut self) {
        self.scope.cancel();
        if let Some(entry) = self.entry.take() {
            let manager = self.manager.clone();
            let held = self.reservation.clone();
            entry.stop.send_replace(true);
            self.runtime.spawn(async move {
                let _ = manager.disconnect_entry(entry).await;
                drop(held);
            });
        }
        // If no worker exists, normal field drop releases the reservation.
    }
}

#[cfg(test)]
mod tests;
