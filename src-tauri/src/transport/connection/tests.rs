use super::*;
use super::super::mock::MockTransport;
use std::sync::atomic::{AtomicUsize,Ordering};

struct ScriptFactory {
    opens:AtomicUsize,
    scripts:parking_lot::Mutex<VecDeque<Vec<TransportResult<Vec<u8>>>>>,
}
#[async_trait]
impl TransportFactory for ScriptFactory{
    async fn open(&self,_:&TransportConfig)->TransportResult<Box<dyn Transport>>{
        self.opens.fetch_add(1,Ordering::SeqCst);
        Ok(Box::new(MockTransport::new(self.scripts.lock().pop_front().unwrap_or_default())))
    }
}
fn definition(id:&str,port:u16)->ConnectionDefinition{
    ConnectionDefinition{id:id.into(),name:id.into(),transport:TransportConfig::Tcp(TcpConfig{
        host:"127.0.0.1".into(),port,..Default::default()
    }),policy:ConnectionPolicy::default()}
}
fn request(priority:Priority)->TransactionRequest{
    TransactionRequest{adu:vec![1,3,0,0,0,1],response_timeout:Duration::from_millis(50),expected_hint:8,priority}
}
#[test]
fn fair_queue_fifo_and_no_starvation(){
    let mut q=FairQueue::default();
    for i in 0..9{q.push(Priority::High,i);}
    for i in 10..16{q.push(Priority::Normal,i);}
    for i in 20..23{q.push(Priority::Low,i);}
    assert_eq!((0..12).map(|_|q.pop().unwrap()).collect::<Vec<_>>(),vec![0,1,2,10,11,20,3,4,5,12,13,21]);
}
#[tokio::test]
async fn duplicate_resource_rejected_and_release_allows_reuse(){
    let m=ConnectionManager::new();
    m.register(definition("a",502)).unwrap();
    assert!(m.register(definition("b",502)).is_err());
    assert!(m.register(definition("a",503)).is_err());
    m.register(definition("b",503)).unwrap();
    m.disconnect("a").await.unwrap();
    m.register(definition("c",502)).unwrap();
    m.disconnect("b").await.unwrap();
    m.disconnect("c").await.unwrap();
    assert!(m.snapshot().is_empty());
}
#[tokio::test]
async fn serial_identity_excludes_framing_and_baud(){
    let m=ConnectionManager::new();
    let port=if cfg!(windows){"COM99"}else{"/dev/modbus-test-port"};
    let serial=|id:&str,baud,mode|ConnectionDefinition{id:id.into(),name:id.into(),transport:TransportConfig::Serial{
        config:SerialPortConfig{port_name:port.into(),baud_rate:baud,..Default::default()},mode},policy:Default::default()};
    m.register(serial("a",9600,SerialMode::Rtu)).unwrap();
    assert!(m.register(serial("b",19200,SerialMode::Ascii)).is_err());
    m.disconnect("a").await.unwrap();
}
#[tokio::test]
async fn persistent_reuses_and_reconnects_without_replaying(){
    let factory=Arc::new(ScriptFactory{opens:AtomicUsize::new(0),scripts:parking_lot::Mutex::new(VecDeque::from([
        vec![Ok(vec![1]),Ok(vec![2]),Err(TransportError::Closed)],vec![Ok(vec![3])]
    ]))});
    let m=ConnectionManager::with_factory(factory.clone());
    m.register(definition("a",502)).unwrap();
    for v in [1,2]{assert_eq!(m.transact("a",request(Priority::Normal)).await.unwrap().result.unwrap(),vec![v]);}
    assert_eq!(factory.opens.load(Ordering::SeqCst),1);
    assert!(m.transact("a",request(Priority::High)).await.unwrap().result.is_err());
    assert_eq!(factory.opens.load(Ordering::SeqCst),1);
    assert_eq!(m.transact("a",request(Priority::Normal)).await.unwrap().result.unwrap(),vec![3]);
    assert_eq!(factory.opens.load(Ordering::SeqCst),2);
    assert_eq!(m.snapshot()[0].reconnect_count,1);
    m.disconnect("a").await.unwrap();
}
#[tokio::test]
async fn on_demand_closes_and_reopens(){
    let factory=Arc::new(ScriptFactory{opens:AtomicUsize::new(0),scripts:parking_lot::Mutex::new(VecDeque::from([vec![Ok(vec![1])],vec![Ok(vec![2])]]))});
    let m=ConnectionManager::with_factory(factory.clone());
    let mut d=definition("a",502);d.policy.mode=ConnectionMode::OnDemand;
    m.register(d).unwrap();
    for v in [1,2]{assert_eq!(m.transact("a",request(Priority::Normal)).await.unwrap().result.unwrap(),vec![v]);}
    assert_eq!(factory.opens.load(Ordering::SeqCst),2);
    assert_eq!(m.snapshot()[0].status,ConnectionStatus::Disconnected);
    assert_eq!(m.snapshot()[0].reconnect_count,0);
    m.disconnect("a").await.unwrap();
}

struct GateFactory {gate:Arc<Semaphore>,entered:Arc<Semaphore>,sent:Arc<AtomicUsize>}
struct GateTransport {gate:Arc<Semaphore>,entered:Arc<Semaphore>,sent:Arc<AtomicUsize>}
#[async_trait]
impl TransportFactory for GateFactory{
    async fn open(&self,c:&TransportConfig)->TransportResult<Box<dyn Transport>>{
        if matches!(c,TransportConfig::Tcp(config) if config.port==503){
            return Ok(Box::new(MockTransport::new(vec![Ok(vec![42])])));
        }
        Ok(Box::new(GateTransport{gate:self.gate.clone(),entered:self.entered.clone(),sent:self.sent.clone()}))
    }
}
#[async_trait]
impl Transport for GateTransport{
    fn name(&self)->String{"gate".into()}
    async fn reset(&mut self)->TransportResult<()>{Ok(())}
    async fn close(&mut self)->TransportResult<()>{Ok(())}
    async fn send_raw(&mut self,_:&[u8])->TransportResult<()>{self.sent.fetch_add(1,Ordering::SeqCst);Ok(())}
    async fn recv_raw(&mut self,_:usize)->TransportResult<Vec<u8>>{
        self.entered.add_permits(1);
        self.gate.acquire().await.map_err(|_|TransportError::Cancelled)?.forget();
        Ok(vec![1])
    }
}
#[tokio::test]
async fn bounded_queue_disconnect_and_independent_endpoint(){
    let gate=Arc::new(Semaphore::new(0));let entered=Arc::new(Semaphore::new(0));let sent=Arc::new(AtomicUsize::new(0));
    let m=Arc::new(ConnectionManager::with_factory(Arc::new(GateFactory{gate:gate.clone(),entered:entered.clone(),sent:sent.clone()})));
    m.register(definition("a",502)).unwrap();m.register(definition("b",503)).unwrap();
    let slow={let m=m.clone();tokio::spawn(async move{let mut r=request(Priority::Normal);r.response_timeout=Duration::from_secs(10);m.transact("a",r).await})};
    tokio::time::timeout(Duration::from_secs(3),entered.acquire()).await.unwrap().unwrap().forget();
    let mut queued=Vec::new();
    for _ in 0..QUEUE_CAPACITY{let m=m.clone();queued.push(tokio::spawn(async move{m.transact("a",request(Priority::Low)).await}));}
    tokio::time::timeout(Duration::from_secs(3),async{
        while m.snapshot()[0].queue_depth!=QUEUE_CAPACITY{tokio::task::yield_now().await;}
    }).await.unwrap();
    assert!(m.transact("a",request(Priority::High)).await.is_err());
    assert_eq!(m.transact("b",request(Priority::Normal)).await.unwrap().result.unwrap(),vec![42]);
    let stopping={let m=m.clone();tokio::spawn(async move{m.disconnect("a").await})};
    tokio::time::timeout(Duration::from_secs(3),async{
        while !*m.entry("a").unwrap().stop.borrow(){tokio::task::yield_now().await;}
    }).await.unwrap();
    gate.add_permits(1);
    assert!(slow.await.unwrap().unwrap().result.is_ok());
    stopping.await.unwrap().unwrap();
    for j in queued{assert!(matches!(j.await.unwrap().unwrap().result,Err(TransportError::Cancelled)));}
    assert_eq!(sent.load(Ordering::SeqCst),1);
    m.disconnect("b").await.unwrap();
}

#[tokio::test]
async fn no_reconnect_when_disabled() {
    let factory=Arc::new(ScriptFactory{opens:AtomicUsize::new(0),scripts:parking_lot::Mutex::new(VecDeque::from([vec![Err(TransportError::Closed)],vec![Ok(vec![9])]]))});
    let m=ConnectionManager::with_factory(factory.clone());
    let mut d=definition("a",502);d.policy.auto_reconnect=false;
    m.register(d).unwrap();
    for _ in 0..2 {assert!(m.transact("a",request(Priority::Normal)).await.unwrap().result.is_err());}
    assert_eq!(factory.opens.load(Ordering::SeqCst),1);
    m.disconnect("a").await.unwrap();
}
#[tokio::test]
async fn dropped_queued_caller_does_not_send() {
    let gate=Arc::new(Semaphore::new(0));let entered=Arc::new(Semaphore::new(0));let sent=Arc::new(AtomicUsize::new(0));
    let m=Arc::new(ConnectionManager::with_factory(Arc::new(GateFactory{gate:gate.clone(),entered:entered.clone(),sent:sent.clone()})));
    m.register(definition("a",502)).unwrap();
    let active={let m=m.clone();tokio::spawn(async move{let mut r=request(Priority::Normal);r.response_timeout=Duration::from_secs(10);m.transact("a",r).await})};
    tokio::time::timeout(Duration::from_secs(3),entered.acquire()).await.unwrap().unwrap().forget();
    let pending={let m=m.clone();tokio::spawn(async move{m.transact("a",request(Priority::High)).await})};
    tokio::time::timeout(Duration::from_secs(3),async{
        while m.snapshot()[0].queue_depth!=1{tokio::task::yield_now().await;}
    }).await.unwrap();
    pending.abort();assert!(pending.await.is_err());
    gate.add_permits(1);assert!(active.await.unwrap().unwrap().result.is_ok());
    tokio::time::timeout(Duration::from_secs(3),async{
        while m.snapshot()[0].queue_depth!=0{tokio::task::yield_now().await;}
    }).await.unwrap();
    assert_eq!(sent.load(Ordering::SeqCst),1);
    m.disconnect("a").await.unwrap();
}
#[tokio::test]
async fn native_tcp_reuses_one_socket_for_two_unit_ids() {
    use tokio::io::{AsyncReadExt,AsyncWriteExt};
    let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port=listener.local_addr().unwrap().port();
    let server=tokio::spawn(async move{
        let (mut socket,_)=listener.accept().await.unwrap();
        for unit in [1u8,2] {
            let mut req=[0u8;12];
            socket.read_exact(&mut req).await.unwrap();
            assert_eq!(req,[0,unit,0,0,0,6,unit,3,0,0,0,1]);
            socket.write_all(&[0,unit,0,0,0,5,unit,3,2,0,42]).await.unwrap();
        }
    });
    let m=ConnectionManager::new();m.register(definition("a",port)).unwrap();
    for unit in [1u8,2]{
        let outcome=m.transact("a",TransactionRequest{adu:vec![0,unit,0,0,0,6,unit,3,0,0,0,1],
            response_timeout:Duration::from_secs(2),expected_hint:11,priority:Priority::Normal}).await.unwrap();
        assert_eq!(outcome.result.unwrap(),vec![0,unit,0,0,0,5,unit,3,2,0,42]);
        assert_eq!(outcome.timing.connect_us.is_some(),unit==1);
        assert!(outcome.timing.rtt_us.is_some());
    }
    m.disconnect("a").await.unwrap();
    tokio::time::timeout(Duration::from_secs(3),server).await.unwrap().unwrap();
}
#[test]
fn registration_outside_runtime_returns_error() {
    assert!(ConnectionManager::new().register(definition("a",502)).is_err());
}
