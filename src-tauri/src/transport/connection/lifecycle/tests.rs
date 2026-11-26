use super::*;
use crate::transport::mock::MockTransport;
use std::sync::atomic::AtomicUsize;

struct Factory{opens:AtomicUsize}
#[async_trait]
impl TransportFactory for Factory{
    async fn open(&self,_:&TransportConfig)->TransportResult<Box<dyn Transport>>{
        self.opens.fetch_add(1,Ordering::SeqCst);
        Ok(Box::new(MockTransport::new(vec![Ok(vec![1]),Ok(vec![2]),Ok(vec![3])])))
    }
}
fn def(id:&str,port:u16)->ConnectionDefinition{
    ConnectionDefinition{id:id.into(),name:id.into(),transport:TransportConfig::Tcp(TcpConfig{port,..Default::default()}),policy:Default::default()}
}
fn req()->TransactionRequest{
    TransactionRequest{adu:vec![1,3,0,0,0,1],response_timeout:Duration::from_millis(50),expected_hint:8,priority:Priority::High}
}
#[tokio::test]
async fn compatible_leases_share_worker_and_stop_is_local(){
    let factory=Arc::new(Factory{opens:AtomicUsize::new(0)});
    let m=Arc::new(ConnectionManager::with_factory(factory.clone()));
    let a=m.acquire(def("scanner",502)).unwrap();
    let b=m.acquire(def("manual",502)).unwrap();
    assert_eq!(a.id(),b.id());
    assert_eq!(a.transact(req(),None).await.unwrap().result.unwrap(),vec![1]);
    a.stop().await;
    assert!(matches!(a.transact(req(),None).await,Err(TransportError::Cancelled)));
    assert_eq!(b.transact(req(),None).await.unwrap().result.unwrap(),vec![2]);
    drop(a);drop(b);
    let c=m.acquire(def("monitor",502)).unwrap();
    assert_eq!(c.transact(req(),None).await.unwrap().result.unwrap(),vec![3]);
    assert_eq!(factory.opens.load(Ordering::SeqCst),1);
    m.shutdown().await.unwrap();
    assert!(m.snapshot().is_empty());
}
#[tokio::test]
async fn incompatible_framing_and_policy_never_replace_existing_worker(){
    let m=Arc::new(ConnectionManager::new());
    let a=m.acquire(def("a",502)).unwrap();
    let mut b=def("b",502);
    if let TransportConfig::Tcp(c)=&mut b.transport{c.framing=TcpFraming::Rtu;}
    assert!(m.acquire(b).is_err());
    let mut b=def("b",502);b.policy.mode=ConnectionMode::OnDemand;
    assert!(m.acquire(b).is_err());
    assert_eq!(m.snapshot().len(),1);
    drop(a);m.shutdown().await.unwrap();
}
#[tokio::test]
async fn expired_idle_workers_retire_but_active_and_explicit_are_pinned(){
    let m=Arc::new(ConnectionManager::new());
    m.register(def("explicit",504)).unwrap();
    let pinned=m.acquire(def("borrowed",504)).unwrap();drop(pinned);
    let active=m.acquire(def("active",503)).unwrap();
    let idle=m.acquire(def("idle",502)).unwrap();
    let idle_id=idle.id().to_owned();drop(idle);
    let entry=m.entry(&idle_id).unwrap();
    entry.lifecycle.lock().idle_since=Some(tokio::time::Instant::now()-IDLE_TCP-Duration::from_secs(1));
    let retired=m.mark_idle_for_cleanup();
    assert_eq!(retired.len(),1);assert_eq!(retired[0].definition.id,idle_id);
    assert!(m.acquire(def("racing",502)).is_err());
    for e in retired{m.disconnect_entry(e).await.unwrap();}
    assert_eq!(m.snapshot().len(),2);
    let replacement=m.acquire(def("replacement",502)).unwrap();
    assert_ne!(replacement.id(),idle_id);
    drop(replacement);drop(active);m.shutdown().await.unwrap();
}
#[tokio::test]
async fn idle_cache_cap_evicts_only_unleased_workers(){
    let m=Arc::new(ConnectionManager::new());
    let held=m.acquire(def("held",1500)).unwrap();
    for port in 2000..2000+IDLE_CACHE_LIMIT as u16+5{
        drop(m.acquire(def("idle",port)).unwrap());
    }
    let entries=m.entries.lock();
    let cached=entries.values().filter(|e|{
        let s=e.lifecycle.lock();s.automatic&&s.users==0&&!*e.stop.borrow()
    }).count();
    assert_eq!(cached,IDLE_CACHE_LIMIT);
    assert!(!*held.entry.stop.borrow());
    drop(entries);
    drop(held);m.shutdown().await.unwrap();
}
#[tokio::test]
async fn stale_lease_cannot_send_via_reused_explicit_id(){
    let m=Arc::new(ConnectionManager::new());
    m.register(def("pinned",502)).unwrap();
    let old=m.acquire(def("borrow",502)).unwrap();
    m.disconnect("pinned").await.unwrap();
    m.register(def("pinned",502)).unwrap();
    assert!(matches!(old.transact(req(),None).await,Err(TransportError::Cancelled)));
    drop(old);
    assert_eq!(m.snapshot().len(),1);
    m.shutdown().await.unwrap();
}
#[tokio::test]
async fn serial_idle_timeout_and_exclusive_discovery_are_distinct(){
    let m=Arc::new(ConnectionManager::new());
    let port=if cfg!(windows){"COM99"}else{"/dev/modbus-c1-test"};
    let d=ConnectionDefinition{id:"serial".into(),name:"serial".into(),transport:TransportConfig::Serial{config:SerialPortConfig{port_name:port.into(),..Default::default()},mode:SerialMode::Rtu},policy:Default::default()};
    assert_eq!(idle_timeout(&d.transport),IDLE_SERIAL);
    let exclusive=m.lease_serial(port).unwrap();assert!(m.acquire(d.clone()).is_err());drop(exclusive);
    let a=m.acquire(d).unwrap();assert!(m.lease_serial(port).is_err());
    let id=a.id().to_owned();drop(a);
    let e=m.entry(&id).unwrap();
    e.lifecycle.lock().idle_since=Some(tokio::time::Instant::now()-IDLE_SERIAL-Duration::from_secs(1));
    for e in m.mark_idle_for_cleanup(){m.disconnect_entry(e).await.unwrap();}
    assert!(m.lease_serial(port).is_ok());
}
struct RecoveryFactory{opens:AtomicUsize}
#[async_trait]
impl TransportFactory for RecoveryFactory{
    async fn open(&self,_:&TransportConfig)->TransportResult<Box<dyn Transport>>{
        let n=self.opens.fetch_add(1,Ordering::SeqCst);
        let replies=if n==0{vec![Err(TransportError::Timeout(Duration::from_millis(50)))]}else{vec![Ok(vec![7])]};
        Ok(Box::new(MockTransport::new(replies)))
    }
}
#[tokio::test]
async fn recovery_deadline_survives_worker_removal_and_config_change(){
    let factory=Arc::new(RecoveryFactory{opens:AtomicUsize::new(0)});
    let m=Arc::new(ConnectionManager::with_factory(factory.clone()));
    let mut d=def("before",502);
    if let TransportConfig::Tcp(c)=&mut d.transport{c.framing=TcpFraming::Rtu;}
    d.policy.recovery_ms=Some(750);
    m.register(d.clone()).unwrap();
    assert!(m.transact("before",req()).await.unwrap().result.is_err());
    m.disconnect("before").await.unwrap();
    assert!(!m.recovery_remaining(&d).unwrap().is_zero());
    let mut other=def("after",502);
    // Even a different framing cannot bypass a previous RTU resource deadline.
    other.policy.recovery_ms=None;
    m.register(other).unwrap();
    let started=Instant::now();
    assert_eq!(m.transact("after",req()).await.unwrap().result.unwrap(),vec![7]);
    assert!(started.elapsed()>=Duration::from_millis(650));
    assert_eq!(factory.opens.load(Ordering::SeqCst),2);
    m.shutdown().await.unwrap();
}

#[tokio::test]
async fn native_tcp_socket_survives_client_release_and_reacquire(){
    use tokio::io::{AsyncReadExt,AsyncWriteExt};
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port=listener.local_addr().unwrap().port();
    let server=tokio::spawn(async move{
        let (mut socket,_)=listener.accept().await.unwrap();
        for unit in [1u8,2,3]{
            let mut request=[0u8;12];
            socket.read_exact(&mut request).await.unwrap();
            assert_eq!(request,[0,unit,0,0,0,6,unit,3,0,0,0,1]);
            socket.write_all(&[0,unit,0,0,0,5,unit,3,2,0,42]).await.unwrap();
        }
        let mut end=[0u8;1];
        assert_eq!(socket.read(&mut end).await.unwrap(),0);
    });
    let m=Arc::new(ConnectionManager::new());
    let a=m.acquire(def("scanner",port)).unwrap();
    let b=m.acquire(def("manual",port)).unwrap();
    let id=a.id().to_owned();
    let read=|unit|TransactionRequest{adu:vec![0,unit,0,0,0,6,unit,3,0,0,0,1],response_timeout:Duration::from_secs(2),expected_hint:11,priority:Priority::High};
    for (lease,unit) in [(&a,1u8),(&b,2)]{
        let out=lease.transact(read(unit),None).await.unwrap();
        assert_eq!(out.result.unwrap(),vec![0,unit,0,0,0,5,unit,3,2,0,42]);
        assert_eq!(out.timing.connect_us.is_some(),unit==1);
    }
    drop(a);drop(b);
    let c=m.acquire(def("monitor",port)).unwrap();
    assert_eq!(c.id(),id);
    let out=c.transact(read(3),None).await.unwrap();
    assert!(out.timing.connect_us.is_none());
    assert_eq!(out.result.unwrap(),vec![0,3,0,0,0,5,3,3,2,0,42]);
    drop(c);m.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3),server).await.unwrap().unwrap();
}

#[tokio::test]
async fn periodic_reaper_releases_expired_cache_without_new_acquire(){
    let m=Arc::new(ConnectionManager::new());
    let lease=m.acquire(def("idle",502)).unwrap();
    let id=lease.id().to_owned();drop(lease);
    let entry=m.entry(&id).unwrap();
    entry.lifecycle.lock().idle_since=Some(tokio::time::Instant::now()-IDLE_TCP-Duration::from_secs(1));
    tokio::time::timeout(Duration::from_secs(3),async{
        while !m.snapshot().is_empty(){tokio::time::sleep(Duration::from_millis(20)).await;}
    }).await.unwrap();
    assert!(m.acquire(def("new",502)).is_ok());
    m.shutdown().await.unwrap();
}
