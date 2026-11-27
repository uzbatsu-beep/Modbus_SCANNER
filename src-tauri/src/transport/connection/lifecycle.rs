//! C.1 shared lifetime foundation. A lease is bound to one worker generation.
//! Automatic workers are cached only while unleased. Explicit workers are pinned.
use super::*;

mod serial_session;
pub use serial_session::{ExclusiveSerialSession, SerialSessionSnapshot};

pub const IDLE_TCP:Duration=Duration::from_secs(60);
pub const IDLE_SERIAL:Duration=Duration::from_secs(30);
pub const IDLE_CACHE_LIMIT:usize=32;
const REAP_INTERVAL:Duration=Duration::from_millis(250);

pub(super) struct LeaseState{
    pub(super) automatic:bool,
    users:usize,
    idle_since:Option<tokio::time::Instant>,
}
impl LeaseState{
    pub(super) fn explicit()->Self{Self{automatic:false,users:0,idle_since:None}}
    fn automatic()->Self{Self{automatic:true,users:1,idle_since:None}}
}

#[derive(Default)]
pub(super) struct RecoveryBook{
    deadlines:parking_lot::Mutex<HashMap<String,tokio::time::Instant>>,
}
impl RecoveryBook{
    pub(super) fn deadline(&self,key:&str)->Option<tokio::time::Instant>{
        let mut deadlines=self.deadlines.lock();
        let now=tokio::time::Instant::now();
        deadlines.retain(|_,until|*until>now);
        deadlines.get(key).copied()
    }
    pub(super) fn record(&self,key:&str,until:tokio::time::Instant){
        let mut deadlines=self.deadlines.lock();
        let now=tokio::time::Instant::now();
        deadlines.retain(|_,value|*value>now);
        let value=deadlines.entry(key.to_owned()).or_insert(until);
        *value=(*value).max(until);
    }
    fn prune(&self){let _=self.deadline("");}
}

fn idle_timeout(config:&TransportConfig)->Duration{
    match config{TransportConfig::Serial{..}=>IDLE_SERIAL,_=>IDLE_TCP}
}

// Resource key already normalizes endpoint aliases. Compare all remaining
// transport and policy fields; never silently replace TLS or framing.
// TCP opening timeout is a connection setting, not a response timeout.
fn compatible(a:&ConnectionDefinition,b:&ConnectionDefinition)->bool{
    fn settings(d:&ConnectionDefinition)->Option<(serde_json::Value,serde_json::Value)>{
        let mut transport=d.transport.clone();
        match &mut transport{
            TransportConfig::Serial{config,..}=>config.port_name.clear(),
            TransportConfig::Tcp(config)=>config.host.clear(),
            TransportConfig::Udp{host,..}=>host.clear(),
        }
        Some((serde_json::to_value(transport).ok()?,serde_json::to_value(&d.policy).ok()?))
    }
    matches!((settings(a),settings(b)),(Some(x),Some(y)) if x==y)
}

impl ConnectionManager{
    /// Acquire or create a compatible connection, atomically with registry
    /// ownership. Existing explicitly registered compatible workers may be
    /// borrowed but remain pinned. An incompatible or retiring worker is busy.
    pub fn acquire(self:&Arc<Self>,mut definition:ConnectionDefinition)->TransportResult<ConnectionLease>{
        let runtime=tokio::runtime::Handle::try_current().map_err(|_|invalid("connection acquisition requires Tokio runtime"))?;
        let resource=definition.resource_key()?;
        let entry={
            let mut entries=self.entries.lock();
            if self.exclusive.lock().contains(&resource){return Err(invalid("physical resource already owned"));}
            if let Some(entry)=entries.values().find(|entry|entry.resource==resource){
                if *entry.stop.borrow()||!compatible(&entry.definition,&definition){
                    return Err(invalid("physical resource busy or incompatible connection settings"));
                }
                let mut state=entry.lifecycle.lock();
                state.users=state.users.checked_add(1).ok_or_else(||invalid("lease count overflow"))?;
                state.idle_since=None;
                entry.clone()
            }else{
                // The generated ID is internal to this worker generation.
                // Caller names and IDs do not split physical ownership.
                definition.id=format!("auto:{:032x}",rand::random::<u128>());
                if entries.contains_key(&definition.id){return Err(invalid("connection id collision"));}
                self.insert_entry(&mut entries,definition,resource,LeaseState::automatic(),&runtime)
            }
        };
        self.start_reaper(&runtime);
        Ok(ConnectionLease{manager:self.clone(),entry,scope:Arc::new(OperationScope::default()),runtime})
    }

    fn start_reaper(self:&Arc<Self>,runtime:&tokio::runtime::Handle){
        if self.reaper_started.swap(true,Ordering::AcqRel){return;}
        let weak=Arc::downgrade(self);
        runtime.spawn(async move{
            loop{
                tokio::time::sleep(REAP_INTERVAL).await;
                let Some(manager)=weak.upgrade()else{break;};
                manager.recovery.prune();
                let retired=manager.mark_idle_for_cleanup();
                for entry in retired{let _=manager.disconnect_entry(entry).await;}
            }
        });
    }

    /// Mark retiring entries while holding the registry lock, before joining
    /// workers. No acquire can race with retirement or reopen before join.
    fn mark_idle_for_cleanup(&self)->Vec<Arc<Entry>>{
        let entries=self.entries.lock();
        let now=tokio::time::Instant::now();
        let mut candidates:Vec<_>=entries.values().filter_map(|entry|{
            let state=entry.lifecycle.lock();
            if !state.automatic||state.users!=0||*entry.stop.borrow(){return None;}
            if entry.slots.available_permits()!=QUEUE_CAPACITY||entry.state.lock().active_requests!=0{return None;}
            state.idle_since.map(|since|(since,entry.clone()))
        }).collect();
        candidates.sort_by(|a,b|a.0.cmp(&b.0).then_with(||a.1.definition.id.cmp(&b.1.definition.id)));
        let excess=candidates.len().saturating_sub(IDLE_CACHE_LIMIT);
        candidates.into_iter().enumerate().filter_map(|(index,(since,entry))|{
            if index<excess||now.saturating_duration_since(since)>=idle_timeout(&entry.definition.transport){
                entry.stop.send_replace(true);
                Some(entry)
            }else{None}
        }).collect()
    }

    /// Remaining recovery deadline is available to future exclusive discovery
    /// adapters. Direct legacy serial callers do not yet consume this state.
    pub fn recovery_remaining(&self,definition:&ConnectionDefinition)->TransportResult<Duration>{
        let key=definition.resource_key()?;
        Ok(self.recovery.deadline(&key).map(|until|until.saturating_duration_since(tokio::time::Instant::now())).unwrap_or_default())
    }

    /// Deterministic shutdown; caller must stop producers before invoking.
    pub async fn shutdown(&self)->TransportResult<()>{
        let entries:Vec<_>={
            let registry=self.entries.lock();
            registry.values().map(|entry|{entry.stop.send_replace(true);entry.clone()}).collect()
        };
        let mut first=None;
        for entry in entries{
            if let Err(error)=self.disconnect_entry(entry).await{if first.is_none(){first=Some(error);}}
        }
        match first{Some(error)=>Err(error),None=>Ok(())}
    }
}

pub struct ConnectionLease{
    manager:Arc<ConnectionManager>,
    entry:Arc<Entry>,
    scope:Arc<OperationScope>,
    runtime:tokio::runtime::Handle,
}
impl ConnectionLease{
    pub fn id(&self)->&str{&self.entry.definition.id}
    pub fn is_cancelled(&self)->bool{self.scope.is_cancelled()}
    pub fn definition(&self)->&ConnectionDefinition{&self.entry.definition}
    pub async fn transact(&self,request:TransactionRequest,validator:Option<ResponseValidator>)->TransportResult<TransactionOutcome>{
        self.manager.transact_entry(self.entry.clone(),request,self.scope.clone(),validator).await
    }
    /// Stop only this client's jobs. Other leases retain access to the worker.
    pub async fn stop(&self){self.scope.stop().await;}
}
impl Drop for ConnectionLease{
    fn drop(&mut self){
        self.scope.cancel();
        {
            let entries=self.manager.entries.lock();
            if entries.get(self.id()).is_some_and(|entry|Arc::ptr_eq(entry,&self.entry)){
                let mut state=self.entry.lifecycle.lock();
                state.users=state.users.saturating_sub(1);
                if state.automatic&&state.users==0{state.idle_since=Some(tokio::time::Instant::now());}
            }
        }
        // Immediate capacity enforcement for quiescent idle workers; the
        // periodic reaper handles jobs still draining at last lease drop.
        let retired=self.manager.mark_idle_for_cleanup();
        if !retired.is_empty(){
            let manager=self.manager.clone();
            self.runtime.spawn(async move{for entry in retired{let _=manager.disconnect_entry(entry).await;}});
        }
    }
}

#[cfg(test)]
mod tests;
