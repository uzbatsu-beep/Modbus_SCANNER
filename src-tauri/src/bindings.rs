//! TypeScript binding generation (E-07).
//!
//! `src/types/ipc.ts` is generated from the Rust types via `ts-rs`. Run
//! `cargo run --bin gen_bindings` after changing any IPC type; the command
//! exports all types and their dependencies into `src/types/generated/` and
//! writes an `index.ts` barrel. The frontend facade keeps only UI constants.

use std::fs;
use std::path::Path;

use ts_rs::TS;

use crate::commands::modbus_cmd::{ManualResponse, ManualTiming, RawFrameRequest, ReadRequest, WriteRequest};
use crate::commands::profiles::ProfileFormat;
use crate::commands::scanner::ExportFormat;
use crate::commands::serial::{SerialPortInfo, TestSerialResult};
use crate::commands::system::AppInfo;
use crate::modbus::common::{SerialMode, TransportKind};
use crate::modbus::decode::{ByteOrder, DataType, DecodeResult, DecodeSpec, DecodedValue, Interpretation, Suggestion};
use crate::modbus::frame::{DeviceIdObject, DeviceIdentification};
use crate::state::Config;
use crate::scanner::types::{
    AddressSpace, RegisterKey, RegisterSpan, DataQuality, ChangeKind, DeviceDiff, DeviceProfile, DeviceRecord, DeviceTarget, IpTargetClass,
    IpTargetStatus, LogFormat, LoggerConfig, LoggerStatus, MonitorSample, MonitorUpdate,
    MonitorValueState, MonitorWatch, ProfileRegister, ProbeResult, Project, RegisterDiff,
    RegisterRange, RegisterScanMode, RegisterScanParams, RegisterValue,
    RotationMode, ScanDiff, ScanEvent, ScanParams, ScanProgress, ScanResults, ScanSnapshot,
    ScanStats, ScanStatus, ScanTarget, SerialFraming, SerialPortParams,
    SerialSweepParams, TagDefinition, TrafficRecord,
};
use crate::transport::serial::{DataBitsSerde, ParitySerde, SerialPortConfig, StopBitsSerde};
use crate::transport::tcp::TcpFraming;
use crate::transport::timing::TransactionTiming;
use crate::scanner::metrics::{MetricRow, CommunicationSnapshot};
use crate::transport::tls::TlsParams;

pub fn export_bindings(manifest_dir: &Path) {
    let out = manifest_dir
        .parent()
        .expect("src-tauri has a parent")
        .join("src/types/generated");
    fs::create_dir_all(&out).expect("create bindings directory");

    macro_rules! export {
        ($($t:ty),+ $(,)?) => {
            $(
                <$t as TS>::export_all_to(&out)
                    .unwrap_or_else(|e| panic!("failed to export {}: {e}", stringify!($t)));
            )+
        };
    }

    export!(
        AddressSpace,
        DataQuality,
        RegisterKey,
        RegisterSpan,
        AppInfo,
        ByteOrder,
        ChangeKind,
        Config,
        DataBitsSerde,
        DataType,
        DecodeResult,
        DecodeSpec,
        DecodedValue,
        DeviceDiff,
        DeviceIdentification,
        DeviceIdObject,
        DeviceProfile,
        DeviceRecord,
        DeviceTarget,
        ExportFormat,
        Interpretation,
        IpTargetClass,
        IpTargetStatus,
        LogFormat,
        LoggerConfig,
        LoggerStatus,
        ManualResponse,
        ManualTiming,
        TransactionTiming,
        MetricRow,
        CommunicationSnapshot,
        MonitorSample,
        MonitorUpdate,
        MonitorValueState,
        MonitorWatch,
        ParitySerde,
        ProfileFormat,
        ProfileRegister,
        ProbeResult,
        Project,
        RawFrameRequest,
        ReadRequest,
        RegisterDiff,
        RegisterRange,
        RegisterScanMode,
        RegisterScanParams,
        RegisterValue,
        RotationMode,
        ScanDiff,
        ScanEvent,
        ScanParams,
        ScanProgress,
        ScanResults,
        ScanSnapshot,
        ScanStats,
        ScanStatus,
        ScanTarget,
        SerialFraming,
        SerialMode,
        SerialPortConfig,
        SerialPortInfo,
        SerialPortParams,
        SerialSweepParams,
        StopBitsSerde,
        Suggestion,
        TagDefinition,
        TcpFraming,
        TestSerialResult,
        TlsParams,
        TrafficRecord,
        TransportKind,
        WriteRequest,
    );

    let mut names: Vec<String> = fs::read_dir(&out)
        .expect("read bindings directory")
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("ts") {
                path.file_stem()?.to_str().map(str::to_string)
            } else {
                None
            }
        })
        .filter(|name| name != "index")
        .collect();
    names.sort();
    let index: String = names
        .iter()
        .map(|name| format!("export * from \"./{name}\";\n"))
        .collect();
    fs::write(out.join("index.ts"), index).expect("write bindings index");
    println!("Generated {} TypeScript bindings in {}", names.len(), out.display());
}
