//! Common connection configuration. Unit IDs, response deadlines and retries
//! are deliberately absent from resource identity.
use std::time::Duration;
use serde::{Deserialize,Serialize};
use crate::modbus::common::SerialMode;
use crate::scanner::types::SerialPortParams;
use crate::transport::{TransportResult,connection::{ConnectionDefinition,ConnectionPolicy,TransportConfig},serial::SerialPortConfig,tcp::{TcpConfig,TcpFraming},tls::TlsParams};

pub const DEFAULT_TCP_CONNECT_TIMEOUT_MS:u32=2000;

/// Five network modes, with TLS expressible only for MBAP.
/// Serial RTU and ASCII are represented by SerialMode.
#[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
#[serde(tag="protocol",rename_all="snake_case")]
pub enum NetworkProtocol{
    TcpMbap,
    TcpRtu,
    TcpTls{tls:TlsParams},
    UdpMbap,
    UdpRtu,
}
impl NetworkProtocol{
    /// Convert existing scanner settings without silently dropping TLS.
    pub fn tcp(framing:TcpFraming,tls:Option<TlsParams>)->TransportResult<Self>{
        match (framing,tls){
            (TcpFraming::Mbap,None)=>Ok(Self::TcpMbap),
            (TcpFraming::Rtu,None)=>Ok(Self::TcpRtu),
            (TcpFraming::Mbap,Some(tls))=>Ok(Self::TcpTls{tls}),
            (TcpFraming::Rtu,Some(_))=>Err(crate::transport::TransportError::Other("TLS requires MBAP framing".into())),
        }
    }
    pub fn default_port(&self)->u16{if matches!(self,Self::TcpTls{..}){802}else{502}}
}

pub fn serial_definition(name:&str,port:&SerialPortParams,mode:SerialMode)->ConnectionDefinition{
    ConnectionDefinition{id:name.into(),name:name.into(),transport:TransportConfig::Serial{
        config:SerialPortConfig{port_name:port.port_name.clone(),baud_rate:port.baud_rate,
            data_bits:port.data_bits,parity:port.parity,stop_bits:port.stop_bits,frame_gap:Duration::from_millis(2)},mode},
        policy:ConnectionPolicy::default()}
}

pub fn network_definition(name:&str,host:&str,port:Option<u16>,protocol:NetworkProtocol,connect_timeout_ms:Option<u32>)->TransportResult<ConnectionDefinition>{
    let port=port.unwrap_or_else(||protocol.default_port());
    let connect_timeout=Duration::from_millis(u64::from(connect_timeout_ms.unwrap_or(DEFAULT_TCP_CONNECT_TIMEOUT_MS)));
    let transport=match protocol{
        NetworkProtocol::TcpMbap=>TransportConfig::Tcp(TcpConfig{host:host.into(),port,connect_timeout,framing:TcpFraming::Mbap,tls:None}),
        NetworkProtocol::TcpRtu=>TransportConfig::Tcp(TcpConfig{host:host.into(),port,connect_timeout,framing:TcpFraming::Rtu,tls:None}),
        NetworkProtocol::TcpTls{tls}=>TransportConfig::Tcp(TcpConfig{host:host.into(),port,connect_timeout,framing:TcpFraming::Mbap,tls:Some(tls)}),
        NetworkProtocol::UdpMbap=>TransportConfig::Udp{host:host.into(),port,mbap:true},
        NetworkProtocol::UdpRtu=>TransportConfig::Udp{host:host.into(),port,mbap:false},
    };
    let definition=ConnectionDefinition{id:name.into(),name:name.into(),transport,policy:ConnectionPolicy::default()};
    definition.resource_key()?;
    Ok(definition)
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn network_modes_are_typed_and_default_connection_timeout_is_independent(){
        for (mode,port,mbap,tls) in [
            (NetworkProtocol::TcpMbap,502,true,false),
            (NetworkProtocol::TcpRtu,502,false,false),
            (NetworkProtocol::TcpTls{tls:TlsParams::default()},802,true,true),
            (NetworkProtocol::UdpMbap,502,true,false),
            (NetworkProtocol::UdpRtu,502,false,false),
        ]{
            let d=network_definition("test","127.0.0.1",None,mode,None).unwrap();
            match d.transport{
                TransportConfig::Tcp(c)=>{assert_eq!(c.port,port);assert_eq!(c.framing==TcpFraming::Mbap,mbap);assert_eq!(c.tls.is_some(),tls);assert_eq!(c.connect_timeout,Duration::from_secs(2));},
                TransportConfig::Udp{port:p,mbap:m,..}=>{assert_eq!(p,port);assert_eq!(m,mbap);assert!(!tls);},
                _=>panic!("network expected"),
            }
        }
        assert!(NetworkProtocol::tcp(TcpFraming::Rtu,Some(TlsParams::default())).is_err());
        assert!(network_definition("bad","127.0.0.1",None,NetworkProtocol::TcpMbap,Some(0)).is_err());
    }
    #[tokio::test]
    async fn tls_and_rtu_cannot_reuse_plain_mbap_resource(){
        use std::sync::Arc;
        use crate::transport::connection::ConnectionManager;
        let manager=Arc::new(ConnectionManager::new());
        let plain=network_definition("plain","127.0.0.1",Some(502),NetworkProtocol::TcpMbap,None).unwrap();
        let lease=manager.acquire(plain).unwrap();
        for mode in [NetworkProtocol::TcpRtu,NetworkProtocol::TcpTls{tls:TlsParams::default()}]{
            let d=network_definition("different","127.0.0.1",Some(502),mode,None).unwrap();
            assert!(manager.acquire(d).is_err());
        }
        drop(lease);manager.shutdown().await.unwrap();
    }
}
