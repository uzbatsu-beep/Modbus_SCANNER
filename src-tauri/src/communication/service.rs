//! Validated PDU service over the Connection Manager. No authorization bypass:
//! callers must apply existing operation/scope policy before submitting.
use std::sync::Arc;
use std::time::Duration;
use crate::modbus::{common::{ModbusError,SerialMode,TransportKind},frame::{Pdu,response::parse_read_values,parse_device_identification}};
use crate::scanner::{engine::{build_adu,verify_adu,next_transaction_id,EngineParams},types::ScanParams};
use crate::transport::{TransportError,TransportResult,connection::{ConnectionManager,ConnectionDefinition,TransportConfig,TransactionRequest,TransactionOutcome,Priority,OperationScope}};

pub struct CommunicationService {
    pub manager: Arc<ConnectionManager>,
}
impl Default for CommunicationService{
    fn default()->Self{Self{manager:Arc::new(ConnectionManager::new())}}
}
pub struct ProtocolRequest {
    pub unit:u8,
    pub pdu:Vec<u8>,
    pub timeout:Duration,
    pub priority:Priority,
}
pub struct ProtocolOutcome{
    pub transaction:TransactionOutcome,
    pub tx_adu:Vec<u8>,
    pub transaction_id:u16,
}
fn invalid(reason:&str)->TransportError{TransportError::Modbus(ModbusError::InvalidFrame(reason.into()))}
fn word(bytes:&[u8])->u16{u16::from_be_bytes([bytes[0],bytes[1]])}
fn validate_request(r:&ProtocolRequest,kind:TransportKind)->TransportResult<()>{
    if r.unit==0 || (matches!(kind,TransportKind::Rtu|TransportKind::Ascii)&&r.unit>247){return Err(invalid("unsupported unit id"));}
    if r.timeout.is_zero()||r.timeout>Duration::from_secs(60){return Err(invalid("response timeout bounds"));}
    let p=&r.pdu;
    let Some(&fc)=p.first() else{return Err(invalid("empty PDU"));};
    if !(1..=253).contains(&p.len()){return Err(invalid("PDU length"));}
    match fc{
        1..=4 if p.len()==5=>{
            let address=word(&p[1..3]);let qty=word(&p[3..5]);let max=if fc<=2{2000}else{125};
            if qty==0||qty>max||u32::from(address)+u32::from(qty)>65536{return Err(invalid("read span"));}
        },
        5 if p.len()==5 && matches!(word(&p[3..5]),0|0xff00)=>{},
        6 if p.len()==5=>{},
        15|16 if p.len()>=6=>{
            let address=word(&p[1..3]);let qty=word(&p[3..5]);
            let (max,count)=if fc==15{(1968,usize::from(qty).div_ceil(8))}else{(123,usize::from(qty)*2)};
            if qty==0||qty>max||u32::from(address)+u32::from(qty)>65536||usize::from(p[5])!=count||p.len()!=6+count{return Err(invalid("write span or byte count"));}
        },
        0x2b if p.len()==4&&p[1]==0x0e&&(1..=4).contains(&p[2])=>{},
        _=>return Err(invalid("unsupported or malformed structured request")),
    }
    Ok(())
}
fn framing(d:&ConnectionDefinition)->(TransportKind,EngineParams){
    let mut params=ScanParams::default();
    let kind=match &d.transport{
        TransportConfig::Serial{mode,..}=>if *mode==SerialMode::Rtu{TransportKind::Rtu}else{TransportKind::Ascii},
        TransportConfig::Tcp(c)=>{params.tcp_framing=c.framing;params.tls=c.tls.clone();TransportKind::Tcp},
        TransportConfig::Udp{mbap,..}=>{params.udp_mbap=*mbap;TransportKind::Udp},
    };
    (kind,EngineParams::from_scan_params(&params))
}
pub(crate) fn verify_payload(pdu:&Pdu,request:&[u8])->TransportResult<()>{
    if let Pdu::Response{function,data}=pdu{
        match *function{
            1..=4=>{parse_read_values(*function,data,word(&request[3..5]))?;},
            5|6|15|16=>{if data.as_slice()!=&request[1..5]{return Err(invalid("write echo mismatch"));}},
            0x2b=>{let id=parse_device_identification(data)?;if id.read_code!=request[2]{return Err(invalid("FC43 read code mismatch"));}},
            _=>return Err(invalid("unsupported response")),
        }
    }
    Ok(())
}
impl CommunicationService{
    /// Register an explicitly selected connection. Reuse only the exact definition;
    /// conflicting framing/TLS/lifetime is rejected instead of silently changed.
    pub fn ensure(&self,definition:ConnectionDefinition)->TransportResult<String>{
        let id=definition.id.clone();
        match self.manager.register(definition.clone()){
            Ok(())=>Ok(id),
            Err(error)=>{
                let old=self.manager.definitions().into_iter().find(|d|d.id==id);
                if let Some(old)=old{
                    if let (Ok(a),Ok(b))=(serde_json::to_value(&old),serde_json::to_value(&definition)){if a==b{return Ok(id);}}
                }
                Err(error)
            }
        }
    }
    pub async fn execute(&self,definition:&ConnectionDefinition,request:ProtocolRequest,scope:Arc<OperationScope>)->TransportResult<ProtocolOutcome>{
        let (kind,engine)=framing(definition);
        validate_request(&request,kind)?;
        if scope.is_cancelled(){return Err(TransportError::Cancelled);}
        let id=self.ensure(definition.clone())?;
        let tx=next_transaction_id();
        let unit=request.unit;
        let pdu=request.pdu;
        let adu=build_adu(kind,unit,&pdu,engine.clone(),tx);
        let verify=move |bytes:&[u8]|{
            let decoded=verify_adu(kind,unit,pdu[0],bytes,tx,engine.clone())?;
            verify_payload(&decoded,&pdu)
        };
        let transaction=self.manager.transact_scoped(&id,TransactionRequest{adu:adu.clone(),response_timeout:request.timeout,expected_hint:8,priority:request.priority},scope,Some(Box::new(verify))).await?;
        Ok(ProtocolOutcome{transaction,tx_adu:adu,transaction_id:tx})
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::transport::tcp::TcpConfig;
    #[test]
    fn validates_spans_and_write_echo(){
        let request=|pdu|ProtocolRequest{unit:1,pdu,timeout:Duration::from_millis(100),priority:Priority::Normal};
        assert!(validate_request(&request(vec![3,255,255,0,2]),TransportKind::Tcp).is_err());
        assert!(validate_request(&request(vec![3,0,0,0,1]),TransportKind::Tcp).is_ok());
        assert!(validate_request(&request(vec![16,0,0,0,2,2,0,1]),TransportKind::Tcp).is_err());
        assert!(verify_payload(&Pdu::Response{function:6,data:vec![0,1,0,4]},&[6,0,1,0,5]).is_err());
        assert!(verify_payload(&Pdu::Response{function:3,data:vec![2,0,42]},&[3,0,1,0,2]).is_err());
        assert!(verify_payload(&Pdu::Exception{function:3,code:2},&[3,0,0,0,1]).is_ok());
    }
    #[tokio::test]
    async fn service_reuses_definition_and_rejects_changed_config(){
        let service=CommunicationService::default();
        let mut d=ConnectionDefinition{id:"one".into(),name:"one".into(),transport:TransportConfig::Tcp(TcpConfig::default()),policy:Default::default()};
        assert_eq!(service.ensure(d.clone()).unwrap(),"one");
        assert_eq!(service.ensure(d.clone()).unwrap(),"one");
        if let TransportConfig::Tcp(c)=&mut d.transport{c.port=503;}
        assert!(service.ensure(d).is_err());
        service.manager.disconnect("one").await.unwrap();
    }
}

#[cfg(test)]
mod loopback_tests{
    use super::*;
    use crate::transport::tcp::TcpConfig;
    use tokio::io::{AsyncReadExt,AsyncWriteExt};
    #[tokio::test]
    async fn valid_exception_keeps_socket_wrong_tid_reopens_and_retains_raw(){
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port=listener.local_addr().unwrap().port();
        let server=tokio::spawn(async move{
            let (mut socket,_)=listener.accept().await.unwrap();
            let mut req=[0u8;12];
            socket.read_exact(&mut req).await.unwrap();
            socket.write_all(&[req[0],req[1],0,0,0,3,req[6],0x83,2]).await.unwrap();
            socket.read_exact(&mut req).await.unwrap();
            socket.write_all(&[req[0],req[1].wrapping_add(1),0,0,0,5,req[6],3,2,0,42]).await.unwrap();
            let (mut socket,_)=listener.accept().await.unwrap();
            socket.read_exact(&mut req).await.unwrap();
            socket.write_all(&[req[0],req[1],0,0,0,5,req[6],3,2,0,42]).await.unwrap();
        });
        let service=CommunicationService::default();
        let d=ConnectionDefinition{id:"loop".into(),name:"loop".into(),transport:TransportConfig::Tcp(TcpConfig{port,..Default::default()}),policy:Default::default()};
        let request=||ProtocolRequest{unit:1,pdu:vec![3,0,0,0,1],timeout:Duration::from_secs(2),priority:Priority::Normal};
        let scope=Arc::new(OperationScope::default());
        let a=service.execute(&d,request(),scope.clone()).await.unwrap();
        assert_eq!(a.transaction.result.unwrap()[7..], [0x83,2]);
        let b=service.execute(&d,request(),scope.clone()).await.unwrap();
        assert!(b.transaction.result.is_err());
        assert_eq!(b.transaction.timing.failure_stage.as_deref(),Some("validation"));
        assert_eq!(b.transaction.rejected_response.unwrap().len(),11);
        let c=service.execute(&d,request(),scope).await.unwrap();
        assert_eq!(c.transaction.result.unwrap()[7..],[3,2,0,42]);
        assert_eq!(service.manager.snapshot()[0].reconnect_count,1);
        service.manager.disconnect("loop").await.unwrap();
        tokio::time::timeout(Duration::from_secs(3),server).await.unwrap().unwrap();
    }
}
