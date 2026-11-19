//! Device profile commands (E-04): reusable register maps matched by FC43 identity.

use serde::Deserialize;
use tauri::{AppHandle, State};

use crate::modbus::decode::{ByteOrder, DataType, DecodeSpec};
use crate::scanner::types::{
    DeviceProfile, ProfileRegister, TagDefinition,
};
use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProfileFormat {
    Csv,
    Json,
}

#[tauri::command]
pub fn get_profiles(state: State<'_, AppState>) -> Vec<DeviceProfile> {
    state.config.lock().profiles.clone()
}

#[tauri::command]
pub fn save_profiles(
    app: AppHandle,
    state: State<'_, AppState>,
    profiles: Vec<DeviceProfile>,
) -> Result<Vec<DeviceProfile>, String> {
    {
        let mut config = state.config.lock();
        config.profiles = profiles.clone();
    }
    let config = state.config.lock().clone();
    crate::commands::config::persist_config(&app, &config)?;
    Ok(profiles)
}

fn data_type(value: &str) -> Result<DataType, String> {
    match value.trim().to_lowercase().as_str() {
        "int16" => Ok(DataType::Int16),
        "uint16" | "" => Ok(DataType::Uint16),
        "int32" => Ok(DataType::Int32),
        "uint32" => Ok(DataType::Uint32),
        "float32" => Ok(DataType::Float32),
        "ascii" => Ok(DataType::Ascii),
        other => Err(format!("unknown data type: {other}")),
    }
}

fn byte_order(value: &str) -> Result<ByteOrder, String> {
    match value.trim().to_lowercase().as_str() {
        "abcd" | "" => Ok(ByteOrder::Abcd),
        "badc" => Ok(ByteOrder::Badc),
        "cdab" => Ok(ByteOrder::Cdab),
        "dcba" => Ok(ByteOrder::Dcba),
        other => Err(format!("unknown byte order: {other}")),
    }
}

fn number(value: &str, default: f64) -> Result<f64, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(default);
    }
    value.parse::<f64>().map_err(|e| format!("bad number '{value}': {e}"))
}

/// CSV columns: vendor,product_code,revision,name,description,function,address,
/// tag_name,data_type,byte_order,multiplier,divider,offset,unit
fn parse_csv(data: &str) -> Result<Vec<DeviceProfile>, String> {
    let lines: Vec<&str> = data.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < 2 {
        return Ok(Vec::new());
    }
    let delimiter = if lines[0].contains(';') { ';' } else { ',' };
    let header: Vec<&str> = lines[0].split(delimiter).map(|s| s.trim()).collect();
fn column<'a>(header: &[&str], row: &'a [&str], name: &str) -> &'a str {
    header
        .iter()
        .position(|h| *h == name)
        .and_then(|i| row.get(i).copied())
        .unwrap_or("")
}

    let mut profiles: Vec<DeviceProfile> = Vec::new();
    for line in &lines[1..] {
        let row: Vec<&str> = line.splitn(header.len(), delimiter).map(|s| s.trim()).collect();
        let vendor = column(&header, &row, "vendor").to_string();
        let product = column(&header, &row, "product_code").to_string();
        let revision = column(&header, &row, "revision").to_string();
        let name = column(&header, &row, "name").to_string();
        let id = format!("{vendor}|{product}|{revision}").to_lowercase().replace(' ', "_");

        let register = ProfileRegister {
            function: column(&header, &row, "function").parse::<u8>().unwrap_or(3),
            address: column(&header, &row, "address")
                .trim_start_matches("0x")
                .parse::<u16>()
                .map_err(|e| format!("bad address: {e}"))?,
            name: column(&header, &row, "tag_name").to_string(),
            spec: DecodeSpec {
                data_type: data_type(column(&header, &row, "data_type"))?,
                byte_order: byte_order(column(&header, &row, "byte_order"))?,
                multiplier: number(column(&header, &row, "multiplier"), 1.0)?,
                divider: number(column(&header, &row, "divider"), 1.0)?,
                offset: number(column(&header, &row, "offset"), 0.0)?,
            },
            unit: {
                let u = column(&header, &row, "unit").to_string();
                if u.is_empty() { None } else { Some(u) }
            },
            description: {
                let d = column(&header, &row, "description").to_string();
                if d.is_empty() { None } else { Some(d) }
            },
        };

        if let Some(profile) = profiles.iter_mut().find(|p| p.id == id) {
            profile.registers.push(register);
        } else {
            profiles.push(DeviceProfile {
                id,
                vendor,
                product_code: product,
                revision,
                name: if name.is_empty() { "Imported profile".into() } else { name },
                description: None,
                registers: vec![register],
            });
        }
    }
    Ok(profiles)
}

#[tauri::command]
pub fn import_profiles(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    format: ProfileFormat,
) -> Result<Vec<DeviceProfile>, String> {
    let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let imported = match format {
        ProfileFormat::Json => {
            let value: serde_json::Value =
                serde_json::from_str(&data).map_err(|e| e.to_string())?;
            let list = value.get("profiles").unwrap_or(&value);
            serde_json::from_value::<Vec<DeviceProfile>>(list.clone())
                .map_err(|e| e.to_string())?
        }
        ProfileFormat::Csv => parse_csv(&data)?,
    };

    let mut config = state.config.lock();
    for profile in imported {
        config
            .profiles
            .retain(|existing| existing.id != profile.id);
        config.profiles.push(profile);
    }
    let profiles = config.profiles.clone();
    drop(config);
    let config = state.config.lock().clone();
    crate::commands::config::persist_config(&app, &config)?;
    Ok(profiles)
}

#[tauri::command]
pub fn create_profile_from_device(
    state: State<'_, AppState>,
    device_key: String,
) -> Result<DeviceProfile, String> {
    let results = state.scanner.results();
    let record = results
        .records
        .iter()
        .find(|r| r.key == device_key)
        .ok_or_else(|| format!("device not found: {device_key}"))?;
    let identity = record
        .device_identification
        .as_ref()
        .ok_or("device has no FC43 identification")?;
    Ok(DeviceProfile::from_identity(identity))
}

#[tauri::command]
pub fn suggest_profile(
    state: State<'_, AppState>,
    device_key: String,
) -> Result<Option<DeviceProfile>, String> {
    let results = state.scanner.results();
    let record = results
        .records
        .iter()
        .find(|r| r.key == device_key)
        .ok_or_else(|| format!("device not found: {device_key}"))?;
    let Some(identity) = record.device_identification.as_ref() else {
        return Ok(None);
    };
    Ok(state
        .config
        .lock()
        .profiles
        .iter()
        .find(|p| p.matches(identity))
        .cloned())
}

#[tauri::command]
pub fn apply_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    device_key: String,
    profile_id: String,
) -> Result<Vec<TagDefinition>, String> {
    let profile = state
        .config
        .lock()
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .cloned()
        .ok_or_else(|| format!("profile not found: {profile_id}"))?;
    let tags = profile.to_tags(&device_key);
    {
        let mut config = state.config.lock();
        config.tags.retain(|tag| tag.device_key != device_key);
        config.tags.extend(tags.iter().cloned());
    }
    let config = state.config.lock().clone();
    crate::commands::config::persist_config(&app, &config)?;
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_csv_profile() {
        let csv = "vendor,product_code,revision,name,function,address,tag_name,data_type,byte_order,multiplier,divider,offset,unit\nAcme,METER,1.2,Acme meter,3,0x0064,Voltage_L1,float32,abcd,1,1,0,V\nAcme,METER,1.2,Acme meter,3,0x0074,Energy,uint32,abcd,1,10,0,kWh\n";
        let profiles = parse_csv(csv).unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].registers.len(), 2);
        assert_eq!(profiles[0].registers[0].name, "Voltage_L1");
        assert_eq!(profiles[0].registers[0].spec.data_type, DataType::Float32);
        assert_eq!(profiles[0].registers[1].address, 0x74);
    }
}
