use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicU64;
use std::time::Instant;
use sled::Db;
use tantivy::Index;
use serde::{Serialize, Deserialize};


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub port: u16,
    pub allowed_ips: Vec<String>,
    pub rate_limit_per_second: u32,
    pub data_path: String,
    pub index_path: String,
    pub upsert_index_buffer: usize,
    pub compact_index_buffer: usize,
    pub default_scan_limit: usize,
    pub max_scan_limit: usize,
    pub payload_limit: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            port: 8080,                                 // Server port number (unitless, TCP port)
            allowed_ips: vec!["127.0.0.1".to_string()], // List of IPs allowed to access restricted endpoints (strings)
            rate_limit_per_second: 10,                  // Max requests per second for rate limiting (requests/second)
            data_path: "./data".to_string(),            // Directory path for Sled database storage (string)
            index_path: "./index".to_string(),          // Directory path for Tantivy index storage (string)
            upsert_index_buffer: 15_000_000,            // Buffer size for Tantivy index writes (bytes, ~14.31 MB)
            compact_index_buffer: 50_000_000,           // Buffer size for Tantivy index compaction (bytes, ~47.68 MB)
            default_scan_limit: 100,                    // Default limit for scan queries (unitless, number of items)
            max_scan_limit: 1000,                       // Maximum limit for scan queries (unitless, number of items)
            payload_limit: 2_097_152,                   // Max HTTP request payload size (bytes, 2 MB)
        }
    }
}

#[derive(Clone)]
pub struct CollectionInfo {
    pub primary_field: String,
    pub index_fields: Vec<String>,
    pub index: Arc<RwLock<Index>>,
}

pub struct AppState {
    pub db: Db,
    pub collections: RwLock<HashMap<String, CollectionInfo>>,
    pub settings: RwLock<Settings>,
    pub start_time: Instant,
    pub request_counter: Arc<AtomicU64>,
}