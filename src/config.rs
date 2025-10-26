use std::fs::{File, OpenOptions};
use std::path::Path;
use env_logger::Builder;
use env_logger::fmt::Formatter;
use log::{error, LevelFilter, Record};
use serde_json::Value;
use crate::models::Settings;
use std::io::Write;

pub fn init_logger() {
    Builder::new()
        .format(|buf: &mut Formatter, record: &Record| {
            let timestamp = buf.timestamp();
            let level = record.level();
            let args = record.args().to_string();
            let module = record.module_path().unwrap_or("unknown");
            let line = record.line().unwrap_or(0);

            let output = if args.starts_with('{') && args.ends_with('}') {
                serde_json::from_str::<Value>(&args)
                    .map(|json_value| serde_json::to_string_pretty(&json_value).unwrap_or_else(|_| args.clone()))
                    .unwrap_or_else(|_| args.clone())
            } else {
                args
            };

            writeln!(buf, "[{}] {} {}:{} - {}", timestamp, level, module, line, output)
        })
        .filter_level(LevelFilter::Info)
        .init();
}

pub fn load_settings() -> Settings {
    let settings_path = "./settings.json";
    let default_settings = Settings::default();

    if !Path::new(settings_path).exists() {
        save_settings_to_file(&default_settings, settings_path).unwrap_or_else(|e| error!("Failed to save defaults: {}", e));
        return default_settings;
    }

    let mut settings: Settings = match File::open(settings_path).and_then(|file| serde_json::from_reader(file).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))) {
        Ok(settings) => settings,
        Err(e) => {
            error!("Failed to deserialize settings: {}", e);
            save_settings_to_file(&default_settings, settings_path).unwrap_or_else(|e| error!("Failed to save defaults: {}", e));
            return default_settings;
        }
    };

    let default_settings_json: Value = serde_json::to_value(&default_settings).expect("Failed to serialize default settings");
    let settings_json: Value = serde_json::to_value(&settings).expect("Failed to serialize settings");


    let mut modified = false;
    if let (Value::Object(mut settings_map), Value::Object(default_map)) = (settings_json, &default_settings_json) {
        for (key, default_value) in default_map {
            let file_value = settings_map.get(key);
            let is_invalid = match (file_value, default_value) {
                (None, _) => true,
                (Some(Value::Number(n)), Value::Number(_)) => {
                    (matches!(key.as_str(), "port" | "rate_limit_per_second" | "upsert_index_buffer" | "compact_index_buffer" | "default_scan_limit" | "max_scan_limit" | "payload_limit") && (n.as_i64() == Some(0) || n.as_f64() == Some(0.0)))
                }
                (Some(Value::String(s)), Value::String(_)) => s.is_empty(),
                (Some(Value::Array(arr)), Value::Array(_)) => arr.is_empty(),
                (Some(_), default) => !matches!(default, Value::Number(_) | Value::String(_) | Value::Array(_)),
            };

            if is_invalid {
                settings_map.insert(key.clone(), default_value.clone());
                modified = true;
            }
        }

        if modified {
            settings = serde_json::from_value(Value::Object(settings_map)).expect("Failed to deserialize merged settings");
        }
    }

    settings
}

pub fn save_settings_to_file(settings: &Settings, path: &str) -> std::io::Result<()> {
    let file = OpenOptions::new().write(true).create(true).truncate(true).open(path)?;
    serde_json::to_writer_pretty(file, &settings)?;
    Ok(())
}