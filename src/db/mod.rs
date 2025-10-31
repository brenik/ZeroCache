pub mod operations;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};
use std::fs;
use log::error;
use tantivy::directory::MmapDirectory;
use tantivy::Index;
use crate::models::CollectionInfo;

pub fn load_existing_collections(index_path: &str) -> HashMap<String, CollectionInfo> {
    let mut collections = HashMap::new();
    let index_base_path = Path::new(index_path);

    if !index_base_path.exists() {
        return collections;
    }

    let Ok(entries) = fs::read_dir(index_base_path) else {
        return collections;
    };

    for entry in entries {
        let Ok(entry) = entry else { continue };

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let key = file_name.to_string();

        let dir = match MmapDirectory::open(&path) {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to open existing index dir for {}: {}", key, e);
                continue;
            }
        };

        let index = match Index::open(dir) {
            Ok(i) => i,
            Err(e) => {
                error!("Failed to open existing index for {}: {}", key, e);
                continue;
            }
        };

        let schema = index.schema();
        let stored_field = schema.fields().find(|(_, field_entry)| field_entry.is_stored());

        let primary_field = if let Some((field, _)) = stored_field {
            schema.get_field_name(field).to_string()
        } else {
            error!("No stored field found for {}", key);
            continue;
        };

        let mut index_fields = vec![];
        for (field, _) in schema.fields() {
            let name = schema.get_field_name(field);
            if name != primary_field && name != "text" {
                index_fields.push(name.to_string());
            }
        }

        let info = CollectionInfo {
            primary_field,
            index_fields,
            index: Arc::new(RwLock::new(index)),
        };
        collections.insert(key, info);
    }

    collections
}