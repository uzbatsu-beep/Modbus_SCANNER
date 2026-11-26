use super::*;
use crate::transport::mock::MockTransport;
use std::sync::atomic::AtomicUsize;

fn port()->&'static str { if cfg!(windows) {"COM99"} else {"/dev/modbus-exclusive-test"} }
fn def(baud:u32,mode:SerialMode)->ConnectionDefinition {
    ConnectionDefinition{id:"serial-test".into(),name:"Serial test".into(),
        transport:TransportConfig::Serial{config:SerialPortConfig{port_name:port().into(),baud_rate:baud,..Default::default()},mode},
        policy:Default::default()}
}
fn req()->TransactionRequest{
    TransactionRequest{adu:vec![1,3,0,0,0,1],response_timeout:Duration::from_millis(50),expected_hint:8,priority:Priority::Low}
}
struct Factory { opens:AtomicUsize, fail_first:bool }
#[async_trait]
impl TransportFactory for Factory{
    async fn open(&self,_:&TransportConfig)->TransportResult<Box<dyn Transport>>{
        let n=self.opens.fetch_add(1,Ordering::SeqCst);
        let replies=if self.fail_first&&n==0{
            vec![Err(TransportError::Timeout(Duration::from_millis(50)))]
        }else{vec![Ok(vec![1]),Ok(vec![2])]};
        Ok(Box::new(MockTransport::new(replies)))
    }
}
#[tokio::test]
async fn idle_cache_handoff_refuses_live_clients_and_pinned_entries(){
    let m=Arc::new(ConnectionManager::new());
    let d=def(9600,SerialMode::Rtu);
    m.register(d.clone()).unwrap();
    assert!(m.exclusive_serial(port(),"sweep").await.is_err());
    m.disconnect(&d.id).await.unwrap();
    let lease=m.acquire(d.clone()).unwrap();
    assert!(m.exclusive_serial(port(),"sweep").await.is_err());
    let old=lease.id().to_owned();drop(lease);
    let mut session=m.exclusive_serial(port(),"sweep").await.unwrap();
    assert!(m.entry(&old).is_err());
    assert!(m.acquire(d.clone()).is_err());
    assert!(m.lease_serial(port()).is_err());
    assert!(m.exclusive_serial(port(),"other").await.is_err());
    session.configure(d.clone()).await.unwrap();
    assert!(m.acquire(d.clone()).is_err());
    session.close().await.unwrap();
    assert!(m.acquire(d).is_ok());
    m.shutdown().await.unwrap();
}
#[tokio::test]
async fn configuration_reuse_and_switch_keep_exclusive_reservation(){
    let f=Arc::new(Factory{opens:AtomicUsize::new(0),fail_first:false});
    let m=Arc::new(ConnectionManager::with_factory(f.clone()));
    let mut s=m.exclusive_serial(port(),"sweep").await.unwrap();
    let d=def(9600,SerialMode::Rtu);
    s.configure(d.clone()).await.unwrap();
    let old=s.snapshot().connection_id.unwrap();
    assert_eq!(s.transact(req(),None).await.unwrap().result.unwrap(),vec![1]);
    s.configure(d.clone()).await.unwrap();
    assert_eq!(s.snapshot().connection_id.as_deref(),Some(old.as_str()));
    assert_eq!(s.transact(req(),None).await.unwrap().result.unwrap(),vec![2]);
    assert_eq!(f.opens.load(Ordering::SeqCst),1);
    s.configure(def(19200,SerialMode::Ascii)).await.unwrap();
    assert!(m.entry(&old).is_err());
    assert_eq!(s.snapshot().configuration_changes,2);
    assert_eq!(s.snapshot().serial_mode,Some(SerialMode::Ascii));
    assert!(m.acquire(d).is_err());
    assert_eq!(s.transact(req(),None).await.unwrap().result.unwrap(),vec![1]);
    assert_eq!(f.opens.load(Ordering::SeqCst),2);
    s.close().await.unwrap();
    assert!(m.snapshot().is_empty());
    assert!(m.lease_serial(port()).is_ok());
}
#[tokio::test]
async fn recovery_survives_exclusive_baud_and_framing_change(){
    let f=Arc::new(Factory{opens:AtomicUsize::new(0),fail_first:true});
    let m=Arc::new(ConnectionManager::with_factory(f.clone()));
    let mut s=m.exclusive_serial(port(),"sweep").await.unwrap();
    let mut d=def(9600,SerialMode::Rtu);d.policy.recovery_ms=Some(750);
    s.configure(d).await.unwrap();
    assert!(s.transact(req(),None).await.unwrap().result.is_err());
    let deadline=m.recovery.deadline(&serial_key(port()).unwrap()).unwrap();
    s.configure(def(19200,SerialMode::Ascii)).await.unwrap();
    assert_eq!(m.recovery.deadline(&serial_key(port()).unwrap()),Some(deadline));
    assert_eq!(s.transact(req(),None).await.unwrap().result.unwrap(),vec![1]);
    assert!(tokio::time::Instant::now()>=deadline);
    assert_eq!(f.opens.load(Ordering::SeqCst),2);
    s.close().await.unwrap();
}
#[tokio::test]
async fn stop_prevents_new_configuration_and_drop_releases_resource(){
    let m=Arc::new(ConnectionManager::new());
    let mut s=m.exclusive_serial(port(),"sweep").await.unwrap();
    s.configure(def(9600,SerialMode::Rtu)).await.unwrap();
    s.stop().await;
    assert!(matches!(s.transact(req(),None).await,Err(TransportError::Cancelled)));
    assert!(matches!(s.configure(def(19200,SerialMode::Ascii)).await,Err(TransportError::Cancelled)));
    drop(s);
    tokio::time::timeout(Duration::from_secs(3),async{
        loop{
            if let Ok(lease)=m.lease_serial(port()){drop(lease);break;}
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    assert!(m.snapshot().is_empty());
}
#[tokio::test]
async fn exclusive_session_cannot_configure_other_resource(){
    let m=Arc::new(ConnectionManager::new());
    let mut s=m.exclusive_serial(port(),"sweep").await.unwrap();
    let mut d=def(9600,SerialMode::Rtu);
    if let TransportConfig::Serial{config,..}=&mut d.transport{
        config.port_name=if cfg!(windows){"COM98"}else{"/dev/modbus-exclusive-other"}.into();
    }
    assert!(s.configure(d.clone()).await.is_err());
    let other=m.acquire(d).unwrap();
    assert!(s.snapshot().connection_id.is_none());
    assert_eq!(m.snapshot().len(),1);
    s.close().await.unwrap();
    drop(other);m.shutdown().await.unwrap();
}

struct GatedFactory {
    rx_entered:Arc<Semaphore>, rx_release:Arc<Semaphore>,
    close_entered:Arc<Semaphore>, close_release:Arc<Semaphore>,
    sent:Arc<AtomicUsize>,
}
struct GatedTransport {
    rx_entered:Arc<Semaphore>, rx_release:Arc<Semaphore>,
    close_entered:Arc<Semaphore>, close_release:Arc<Semaphore>,
    sent:Arc<AtomicUsize>,
}
#[async_trait]
impl TransportFactory for GatedFactory {
    async fn open(&self,_:&TransportConfig)->TransportResult<Box<dyn Transport>>{
        Ok(Box::new(GatedTransport{rx_entered:self.rx_entered.clone(),rx_release:self.rx_release.clone(),
            close_entered:self.close_entered.clone(),close_release:self.close_release.clone(),sent:self.sent.clone()}))
    }
}
#[async_trait]
impl Transport for GatedTransport {
    fn name(&self)->String{"serial gate".into()}
    async fn reset(&mut self)->TransportResult<()>{Ok(())}
    async fn send_raw(&mut self,_:&[u8])->TransportResult<()>{self.sent.fetch_add(1,Ordering::SeqCst);Ok(())}
    async fn recv_raw(&mut self,_:usize)->TransportResult<Vec<u8>>{
        self.rx_entered.add_permits(1);
        self.rx_release.acquire().await.map_err(|_|TransportError::Cancelled)?.forget();
        Ok(vec![1])
    }
    async fn close(&mut self)->TransportResult<()>{
        self.close_entered.add_permits(1);
        self.close_release.acquire().await.map_err(|_|TransportError::Cancelled)?.forget();
        Ok(())
    }
}
fn gated()->Arc<GatedFactory>{
    Arc::new(GatedFactory{rx_entered:Arc::new(Semaphore::new(0)),rx_release:Arc::new(Semaphore::new(0)),
        close_entered:Arc::new(Semaphore::new(0)),close_release:Arc::new(Semaphore::new(0)),sent:Arc::new(AtomicUsize::new(0))})
}
async fn entered(s:&Semaphore){tokio::time::timeout(Duration::from_secs(3),s.acquire()).await.unwrap().unwrap().forget();}
#[tokio::test]
async fn cancelled_idle_handoff_holds_reservation_until_worker_join(){
    let f=gated();let m=Arc::new(ConnectionManager::with_factory(f.clone()));
    let d=def(9600,SerialMode::Rtu);
    let lease=m.acquire(d.clone()).unwrap();
    f.rx_release.add_permits(1);
    let mut request=req();request.response_timeout=Duration::from_secs(3);
    assert!(lease.transact(request,None).await.unwrap().result.is_ok());
    drop(lease);
    let task={let m=m.clone();tokio::spawn(async move{m.exclusive_serial(port(),"sweep").await})};
    entered(&f.close_entered).await;
    // The worker is closing, and the exclusive claim was already installed.
    assert!(m.acquire(d.clone()).is_err());
    assert!(m.lease_serial(port()).is_err());
    task.abort();assert!(task.await.err().unwrap().is_cancelled());
    assert!(m.acquire(d.clone()).is_err());
    assert!(m.lease_serial(port()).is_err());
    f.close_release.add_permits(1);
    tokio::time::timeout(Duration::from_secs(3),async{
        loop{
            if let Ok(lease)=m.lease_serial(port()){drop(lease);break;}
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    assert!(m.snapshot().is_empty());
    assert!(m.acquire(d).is_ok());
    m.shutdown().await.unwrap();
}
#[tokio::test]
async fn exclusive_stop_finishes_active_rx_and_cancels_waiting_request(){
    let f=gated();let m=Arc::new(ConnectionManager::with_factory(f.clone()));
    let mut s=m.exclusive_serial(port(),"sweep").await.unwrap();
    s.configure(def(9600,SerialMode::Rtu)).await.unwrap();
    let s=Arc::new(s);
    let active={let s=s.clone();tokio::spawn(async move{
        let mut r=req();r.response_timeout=Duration::from_secs(5);s.transact(r,None).await
    })};
    entered(&f.rx_entered).await;
    let pending={let s=s.clone();tokio::spawn(async move{s.transact(req(),None).await})};
    tokio::time::timeout(Duration::from_secs(3),async{
        while m.snapshot()[0].queue_depth!=1{tokio::task::yield_now().await;}
    }).await.unwrap();
    let stopping={let s=s.clone();tokio::spawn(async move{s.stop().await})};
    let cancelled=tokio::time::timeout(Duration::from_secs(3),pending).await.unwrap().unwrap();
    assert!(matches!(cancelled,Err(TransportError::Cancelled)));
    assert!(!stopping.is_finished());
    assert!(m.lease_serial(port()).is_err());
    f.rx_release.add_permits(1);
    assert_eq!(active.await.unwrap().unwrap().result.unwrap(),vec![1]);
    stopping.await.unwrap();
    assert_eq!(f.sent.load(Ordering::SeqCst),1);
    let s=Arc::try_unwrap(s).ok().unwrap();
    f.close_release.add_permits(1);
    s.close().await.unwrap();
    assert!(m.snapshot().is_empty());
    assert!(m.lease_serial(port()).is_ok());
}
