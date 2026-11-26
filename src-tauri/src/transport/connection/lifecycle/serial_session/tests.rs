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
