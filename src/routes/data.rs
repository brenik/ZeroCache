use std::collections::HashMap;
use std::path::PathBuf;
use tantivy::query::QueryParser;
use tantivy::collector::TopDocs;
use tantivy::schema::{Field, FieldType};
use tantivy::index::SegmentId;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_json::{json, Value};
use std::io::Cursor;
use std::path::Path;
use std::fs;
use log::{info, error, warn};
use tantivy::{Index, TantivyDocument};
use tantivy::directory::MmapDirectory;
use tantivy::{IndexWriter};
use tantivy::schema::Value as TantivyValue;
use std::sync::{Arc, RwLock};

use crate::models::{AppState, CollectionInfo};
use crate::db::operations::{get_object_id_from_json, create_tantivy_schema};

pub async fn set_item(
    app_data: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Bytes,
) -> impl Responder {
    let key = path.into_inner();
    info!("POST to /data/{} with streaming body", key);
    let upsert_field_header = req.headers().get("X-Upsert-Field")
        .and_then(|header| header.to_str().ok())
        .map(|s| s.split(',').map(|f| f.trim().to_string()).collect::<Vec<_>>())
        .unwrap_or_default();
    let (provided_primary, provided_index_fields) = if !upsert_field_header.is_empty() {
        (upsert_field_header[0].clone(), upsert_field_header[1..].to_vec())
    } else {
        let reader_clone = Cursor::new(&body);
        let de_clone = serde_json::Deserializer::from_reader(reader_clone);
        if let Some(Ok(first_item)) = de_clone.into_iter::<Value>().next() {
            if let Some(field) = first_item.as_object().and_then(|obj| obj.keys().next()) {
                (field.clone(), vec![])
            } else {
                return HttpResponse::BadRequest().json(json!({"error": "No primary key field found in item"}));
            }
        } else {
            return HttpResponse::BadRequest().json(json!({"error": "Failed to parse item for primary key"}));
        }
    };
    let mut collections_lock = app_data.collections.write().unwrap();
    let collection_info = if !collections_lock.contains_key(&key) {
        let settings = app_data.settings.read().unwrap();
        let index_dir_path = Path::new(&settings.index_path).join(&key);
        if let Err(e) = fs::create_dir_all(&index_dir_path) {
            error!("Failed to create index directory for {}: {}", key, e);
            return HttpResponse::InternalServerError().json(json!({"error": "Failed to create index directory"}));
        }
        let dir = match MmapDirectory::open(&index_dir_path) {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to open index directory for {}: {}", key, e);
                return HttpResponse::InternalServerError().json(json!({"error": "Failed to open index directory"}));
            }
        };
        let schema = create_tantivy_schema(&provided_primary, &provided_index_fields);
        let index = match Index::open_or_create(dir, schema) {
            Ok(i) => i,
            Err(e) => {
                error!("Failed to create index for {}: {}", key, e);
                return HttpResponse::InternalServerError().json(json!({"error": "Failed to create search index"}));
            }
        };
        let info = CollectionInfo {
            primary_field: provided_primary.clone(),
            index_fields: provided_index_fields.clone(),
            index: Arc::new(RwLock::new(index)),
        };
        collections_lock.insert(key.clone(), info.clone());
        info
    } else {
        let existing = collections_lock.get(&key).unwrap();
        if provided_primary != existing.primary_field {
            return HttpResponse::BadRequest().json(json!({"error": "Primary field mismatch with existing collection"}));
        }
        existing.clone()
    };
    drop(collections_lock);
    let primary_field = &collection_info.primary_field;
    let initial_index_fields = &collection_info.index_fields;
    let reader = Cursor::new(&body);
    let de = serde_json::Deserializer::from_reader(reader);
    let stream = de.into_iter::<Value>();
    let schema = collection_info.index.read().unwrap().schema();
    let primary_f = match schema.get_field(primary_field) {
        Ok(field) => field,
        Err(_) => {
            error!("Primary field '{}' not found in schema", primary_field);
            return HttpResponse::InternalServerError().json(json!({
            "error": "Schema error: primary field not found"
        }));
        }
    };
    let text_f = schema.get_field("text").unwrap();
    let settings = app_data.settings.read().unwrap();
    let min_buffer = settings.upsert_index_buffer;
    let body_size = body.len();
    let buffer_size = std::cmp::max(min_buffer, body_size);
    info!("Using dynamic buffer size: {} bytes (body size: {} bytes)", buffer_size, body_size);
    let index_lock = collection_info.index.clone();

    let index_guard = match index_lock.write() {
        Ok(guard) => guard,
        Err(e) => {
            error!("Failed to acquire index write lock: {}", e);
            return HttpResponse::InternalServerError().json(json!({
            "error": "Index temporarily unavailable"
        }));
        }
    };

    let mut index_writer = match index_guard.writer(buffer_size) {
        Ok(writer) => writer,
        Err(e) => {
            error!("Failed to create Index writer: {}", e);
            return HttpResponse::InternalServerError().json(json!({
            "error": "Failed to initialize search index writer"
        }));
        }
    };
    let tree = match app_data.db.open_tree(&key) {
        Ok(t) => t,
        Err(e) => {
            error!("Failed to open Sled tree '{}': {}", key, e);
            return HttpResponse::InternalServerError().json(json!({"error": "Failed to open database collection"}));
        }
    };
    let mut count = 0;
    let mut errors = 0;
    for item_result in stream {
        match item_result {
            Ok(item) => {
                if let Some(object_id) = get_object_id_from_json(&item, primary_field) {
                    match serde_json::to_vec(&item) {
                        Ok(serialized_data) => {
                            if let Err(e) = tree.insert(object_id.as_bytes(), serialized_data) {
                                error!("Failed to insert into Sled: {}", e);
                                errors += 1;
                                continue;
                            }
                        }
                        Err(e) => {
                            error!("Failed to serialize item: {}", e);
                            errors += 1;
                            continue;
                        }
                    }

                    let mut tantivy_doc = TantivyDocument::default();
                    tantivy_doc.add_text(primary_f, &object_id);

                    for field_name in initial_index_fields {
                        if let Some(value) = item.get(field_name) {
                            if let Ok(field) = schema.get_field(field_name) {
                                tantivy_doc.add_text(field, &value.to_string());
                            }
                        }
                    }

                    let mut text_content = String::new();
                    for field_name in &provided_index_fields {
                        if let Some(value) = item.get(field_name) {
                            text_content.push_str(&value.to_string());
                            text_content.push(' ');
                        }
                    }
                    tantivy_doc.add_text(text_f, text_content.trim());
                    if let Err(e) = index_writer.add_document(tantivy_doc) {
                        error!("Failed to add document to Index: {}", e);
                        errors += 1;
                        continue;
                    }
                    count += 1;

                    if count % 1000 == 0 {
                        if let Err(e) = index_writer.commit() {
                            error!("Failed to commit Index batch: {}", e);
                        } else {
                            let buffer_size = app_data.settings.read().unwrap().upsert_index_buffer;

                            let new_writer_result = match index_lock.write() {
                                Ok(guard) => guard.writer(buffer_size),
                                Err(e) => {
                                    warn!("Lock error during periodic commit: {}", e);
                                    continue;
                                }
                            };

                            match new_writer_result {
                                Ok(new_writer) => index_writer = new_writer,
                                Err(e) => {
                                    error!("Failed to create new writer after commit: {}", e);
                                    return HttpResponse::InternalServerError().json(json!({"error": "Failed to maintain search index"}));
                                }
                            }
                        }
                    }
                } else {
                    error!("Item missing primary key field '{}': {:?}", primary_field, item);
                    errors += 1;
                }
            }
            Err(e) => {
                error!("Stream deserialization error: {}", e);
                return HttpResponse::InternalServerError().json(json!({
                    "error": "Failed to parse JSON stream",
                    "details": e.to_string()
                }));
            }
        }
    }

    if let Err(e) = index_writer.commit() {
        error!("Failed to commit final Index batch: {}", e);
        warn!("Possible disk space issue or index corruption");
        return HttpResponse::InsufficientStorage().json(json!({
        "error": "Failed to commit search index - disk may be full",
        "processed": count,
        "errors": errors,
        "details": e.to_string()
    }));
    }

    if let Err(e) = tree.flush() {
        error!("Failed to flush Sled tree: {}", e);
        warn!("Data may not be persisted to disk - possible disk space issue");
    }
    info!("Completed upsert: {} items processed, {} errors", count, errors);
    HttpResponse::Ok().json(json!({
        "success": true,
        "operation": "upsert",
        "count": count,
        "errors": errors,
        "collection": key
    }))
}

pub async fn get_item(
    app_data: web::Data<AppState>,
    _req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    let key = path.into_inner();
    info!("GET for key '{}'", key);

    let collections_lock = app_data.collections.read().unwrap();
    let collection_info_opt = collections_lock.get(&key);
    if collection_info_opt.is_none() {
        return HttpResponse::NotFound().json(json!({"error": "Collection not found"}));
    }
    let collection_info = collection_info_opt.unwrap().clone();
    drop(collections_lock);

    let primary_field = &collection_info.primary_field;
    let indexed_fields = &collection_info.index_fields;
    let index_lock = collection_info.index.clone();
    let index = match index_lock.read() {
        Ok(guard) => guard,
        Err(e) => {
            error!("Failed to acquire index read lock: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "error": "Index temporarily unavailable"
            }));
        }
    };

    let reader = match index.reader() {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to create Index reader: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "error": "Failed to initialize search index reader"
            }));
        }
    };
    let searcher = reader.searcher();
    let schema = searcher.schema();
    let mut results = Vec::new();
    let mut range_filters: HashMap<String, (Option<f64>, Option<f64>)> = HashMap::new();

    let settings = app_data.settings.read().unwrap();
    let effective_limit = query.get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(settings.default_scan_limit);
    let offset = query.get("offset")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(0);
    drop(settings);

    if let Some(id_val) = query.get(primary_field) {
        if let Ok(tree) = app_data.db.open_tree(&key) {
            if let Ok(Some(item_bytes)) = tree.get(id_val.as_bytes()) {
                if let Ok(item_json) = serde_json::from_slice::<Value>(&item_bytes) {
                    results.push(item_json);
                }
            }
        }
    } else {
        let mut query_parts = Vec::new();
        let mut has_filters = false;

        for (param_key, param_value) in query.iter() {
            if param_key == "limit" || param_key == "offset" ||
                param_key == "sort_by" || param_key == "sort_order" ||
                param_key == "q" {
                continue;
            }

            if let Some(field_name) = param_key.strip_prefix("filter_min_") {
                if !indexed_fields.contains(&field_name.to_string()) {
                    return HttpResponse::BadRequest().json(json!({
                        "error": format!("Field '{}' is not indexed. Create an index for filtering.", field_name)
                    }));
                }
                if let Ok(min_val) = param_value.parse::<f64>() {
                    range_filters.entry(field_name.to_string())
                        .or_insert((None, None))
                        .0 = Some(min_val);
                }
                has_filters = true;
            } else if let Some(field_name) = param_key.strip_prefix("filter_max_") {
                if !indexed_fields.contains(&field_name.to_string()) {
                    return HttpResponse::BadRequest().json(json!({
                        "error": format!("Field '{}' is not indexed. Create an index for filtering.", field_name)
                    }));
                }
                if let Ok(max_val) = param_value.parse::<f64>() {
                    range_filters.entry(field_name.to_string())
                        .or_insert((None, None))
                        .1 = Some(max_val);
                }
                has_filters = true;
            } else {
                if !indexed_fields.contains(param_key) {
                    return HttpResponse::BadRequest().json(json!({
                        "error": format!("Field '{}' is not indexed. Create an index for filtering.", param_key)
                    }));
                }
                query_parts.push(format!("{}:\"{}\"", param_key, param_value));
                has_filters = true;
            }
        }

        if let Some(q_val) = query.get("q") {
            query_parts.push(q_val.clone());
            has_filters = true;
        }

        if has_filters {
            let query_string = if query_parts.is_empty() {
                "*".to_string()
            } else {
                query_parts.join(" AND ")
            };

            let all_fields: Vec<Field> = schema.fields()
                .filter(|(_, field_entry)| {
                    matches!(*field_entry.field_type(), FieldType::Str(_))
                })
                .map(|(field, _)| field)
                .collect();

            let query_parser = QueryParser::for_index(&index, all_fields);
            let tantivy_query = match query_parser.parse_query(&query_string) {
                Ok(query) => query,
                Err(e) => {
                    warn!("Failed to parse search query '{}': {}", query_string, e);
                    return HttpResponse::BadRequest().json(json!({
                        "error": "Invalid search query",
                        "details": e.to_string()
                    }));
                }
            };

            // let settings = app_data.settings.read().unwrap();
            // let limit = query.get("limit")
            //     .and_then(|s| s.parse::<usize>().ok())
            //     .unwrap_or(settings.max_scan_limit);

            let top_docs = match searcher.search(&tantivy_query, &TopDocs::with_limit(effective_limit)) {
                Ok(docs) => docs,
                Err(e) => {
                    error!("Search execution failed: {}", e);
                    return HttpResponse::InternalServerError().json(json!({
                        "error": "Search execution failed"
                    }));
                }
            };

            if let Ok(tree) = app_data.db.open_tree(&key) {
                let primary_f = match schema.get_field(primary_field) {
                    Ok(field) => field,
                    Err(_) => {
                        error!("Primary field '{}' not found in schema", primary_field);
                        return HttpResponse::InternalServerError().json(json!({
                            "error": "Schema error: primary field not found"
                        }));
                    }
                };

                for (_score, doc_address) in top_docs {
                    let retrieved_doc: TantivyDocument = match searcher.doc(doc_address) {
                        Ok(doc) => doc,
                        Err(e) => {
                            error!("Failed to retrieve document: {}", e);
                            continue;
                        }
                    };

                    if let Some(field_value) = retrieved_doc.get_first(primary_f) {
                        let object_id_val = if let Some(text) = field_value.as_str() {
                            text.to_string()
                        } else {
                            error!("Primary field value is not a string");
                            continue;
                        };

                        if !object_id_val.is_empty() {
                            if let Ok(Some(item_bytes)) = tree.get(object_id_val.as_bytes()) {
                                if let Ok(item_json) = serde_json::from_slice::<Value>(&item_bytes) {
                                    results.push(item_json);
                                } else {
                                    error!("Failed to deserialize item for ID: {}", object_id_val);
                                }
                            } else {
                                warn!("Item not found in Sled for ID: {}", object_id_val);
                            }
                        } else {
                            error!("Could not extract ID from document");
                        }
                    }
                }
            }
        } else {
            // let settings = app_data.settings.read().unwrap();
            // let limit = query.get("limit")
            //     .and_then(|s| s.parse::<usize>().ok())
            //     .unwrap_or(settings.max_scan_limit);
            // let offset = query.get("offset")
            //     .and_then(|s| s.parse::<usize>().ok())
            //     .unwrap_or(0);

            if let Ok(tree) = app_data.db.open_tree(&key) {
                let mut count = 0;
                let mut skipped = 0;

                for item_result in tree.iter() {
                    if let Ok((_key_bytes, value_bytes)) = item_result {
                        if skipped < offset {
                            skipped += 1;
                            continue;
                        }

                        if let Ok(item_json) = serde_json::from_slice::<Value>(&value_bytes) {
                            results.push(item_json);
                            count += 1;
                            if count >= effective_limit {
                                break;
                            }
                        } else {
                            error!("Failed to deserialize item during full scan");
                        }
                    } else {
                        error!("Failed to read item during full scan");
                    }
                }
                info!("Full scan returned {} items (offset: {}, effective_limit: {})", results.len(), offset, effective_limit);
            }
        }
    }

    if !range_filters.is_empty() {
        results.retain(|item| {
            range_filters.iter().all(|(field_name, (min_opt, max_opt))| {
                if let Some(field_value) = item.get(field_name) {
                    let value_f64 = match field_value {
                        Value::Number(n) => n.as_f64(),
                        Value::String(s) => s.parse::<f64>().ok(),
                        _ => None,
                    };

                    if let Some(val) = value_f64 {
                        let min_ok = min_opt.map_or(true, |min| val >= min);
                        let max_ok = max_opt.map_or(true, |max| val <= max);
                        min_ok && max_ok
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
        });
    }

    if let Some(sort_field_name) = query.get("sort_by") {
        let sort_order = query.get("sort_order").map(|s| s.as_str()).unwrap_or("asc");

        results.sort_by(|a, b| {
            let a_val = a.get(sort_field_name);
            let b_val = b.get(sort_field_name);

            let cmp = match (a_val, b_val) {
                (Some(Value::Number(a_num)), Some(Value::Number(b_num))) => {
                    let a_f64 = a_num.as_f64().unwrap_or(0.0);
                    let b_f64 = b_num.as_f64().unwrap_or(0.0);
                    a_f64.partial_cmp(&b_f64).unwrap_or(std::cmp::Ordering::Equal)
                }
                (Some(Value::String(a_str)), Some(Value::String(b_str))) => {
                    match (a_str.parse::<f64>(), b_str.parse::<f64>()) {
                        (Ok(a_num), Ok(b_num)) => {
                            a_num.partial_cmp(&b_num).unwrap_or(std::cmp::Ordering::Equal)
                        }
                        _ => a_str.cmp(b_str)
                    }
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            };

            if sort_order == "desc" { cmp.reverse() } else { cmp }
        });
    }

    if !results.is_empty() {
        HttpResponse::Ok().json(json!({
            key: results,
            "total": results.len(),
            "limit": effective_limit,
            "offset": offset,
            "query_type": if query.contains_key(primary_field) {
                "direct_lookup"
            } else if query.contains_key("q") || query.iter().any(|(k, _)| k != "limit" && k != "offset" && k != "sort_by" && k != "sort_order") {
                "index_search"
            } else {
                "full_scan"
            }
        }))
    } else {
        HttpResponse::NotFound().json(json!({
            "error": format!("No items found for key '{}'", key),
            "query": query.into_inner()
        }))
    }
}

pub async fn delete_item(
    app_data: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<HashMap<String, String>>,
) -> impl Responder {
    let key = path.into_inner();

    // Scenario 1: Delete entire collection
    if query.is_empty() {
        if req.headers().get("X-Confirm-Purge").map(|v| v.to_str().unwrap_or("")) != Some("true") {
            return HttpResponse::BadRequest().json(json!({
                "error": "Collection deletion requires X-Confirm-Purge: true header"
            }));
        }

        info!("DELETE entire collection '{}'", key);
        app_data.db.drop_tree(&key).ok();
        app_data.collections.write().unwrap().remove(&key);

        let settings = app_data.settings.read().unwrap();
        let index_path = Path::new(&settings.index_path).join(&key);
        fs::remove_dir_all(&index_path).ok();

        return HttpResponse::Ok().json(json!({"message": format!("Deleted collection '{}'", key)}));
    }

    // Scenario 2: Delete by primary key
    let collections = app_data.collections.read().unwrap();
    let info = match collections.get(&key) {
        Some(i) => i.clone(),
        None => return HttpResponse::NotFound().json(json!({"error": "Collection not found"})),
    };
    let primary_field_name = &info.primary_field;
    drop(collections);

    let primary_id = match query.get(primary_field_name) {
        Some(id) => id,
        None => return HttpResponse::BadRequest().json(json!({
            "error": format!("Primary key '{}' required", primary_field_name)
        })),
    };

    info!("DELETE item with {}={} from '{}'", primary_field_name, primary_id, key);

    let tree = app_data.db.open_tree(&key).ok().unwrap();

    match tree.remove(primary_id.as_bytes()) {
        Ok(Some(_)) => {
            let index = info.index.read().unwrap();
            let schema = index.schema();
            let primary_field = schema.get_field(primary_field_name).unwrap();

            let settings = app_data.settings.read().unwrap();
            let mut writer: IndexWriter = index.writer(settings.upsert_index_buffer).ok().unwrap();

            writer.delete_term(tantivy::Term::from_field_text(primary_field, primary_id));
            writer.commit().ok();

            HttpResponse::Ok().json(json!({
                "deleted": 1,
                "collection": key,
                "id": primary_id
            }))
        }
        Ok(None) => {
            HttpResponse::NotFound().json(json!({
                "error": "Item not found"
            }))
        }
        Err(e) => {
            error!("Delete error: {}", e);
            HttpResponse::InternalServerError().json(json!({"error": "Delete failed"}))
        }
    }
}


pub async fn delete_all(
    app_data: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    info!("PURGE request");
    if req.headers().get("X-Confirm-Purge").map(|v| v.to_str().unwrap_or("")) != Some("true") {
        return HttpResponse::BadRequest().json(json!({"error": "Confirmation required"}));
    }
    let settings = app_data.settings.read().unwrap().clone();

    let tree_names = app_data.db.tree_names();
    for tree_name in tree_names {
        let tree_name_str = String::from_utf8(tree_name.to_vec()).unwrap_or_default();
        if tree_name_str == "__sled__default" {
            info!("Skipping reserved tree: {:?}", tree_name);
            continue;
        }
        if let Err(e) = app_data.db.drop_tree(&tree_name) {
            error!("Failed to drop tree {:?}: {}", tree_name, e);
        } else {
            info!("Successfully dropped tree: {:?}", tree_name);
        }
    }

    let index_path = PathBuf::from(&settings.index_path);
    if index_path.exists() {
        if let Err(e) = fs::remove_dir_all(&index_path) {
            error!("Failed to remove index directory: {}", e);
        }
        if let Err(e) = fs::create_dir_all(&index_path) {
            error!("Failed to recreate index directory: {}", e);
        }
    }
    let mut collections_lock = app_data.collections.write().unwrap();
    collections_lock.clear();
    HttpResponse::Ok().json(json!({"message": "Purged all collections and search index"}))
}

pub async fn compact(
    app_data: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    info!("COMPACT request");
    if req.headers().get("X-Confirm-Compact").map(|v| v.to_str().unwrap_or("")) != Some("true") {
        return HttpResponse::BadRequest().json(json!({"error": "Confirmation required"}));
    }
    let settings = app_data.settings.read().unwrap();
    let mut messages = vec![];
    if let Err(e) = app_data.db.flush_async().await {
        error!("Failed to flush Sled: {}", e);
        messages.push("Failed to compact DB".to_string());
    } else {
        messages.push("DB compacted".to_string());
    }

    let collection_keys: Vec<String> = app_data.collections.read().unwrap().keys().cloned().collect();
    for key in collection_keys {
        let collections_lock = app_data.collections.read().unwrap();
        let info_opt = collections_lock.get(&key);
        if let Some(info) = info_opt {
            let index_lock = info.index.clone();
            drop(collections_lock);
            let index_guard = match index_lock.write() {
                Ok(guard) => guard,
                Err(e) => {
                    error!("Failed to acquire lock for {}: {}", key, e);
                    messages.push(format!("Failed to compact {}: lock error", key));
                    continue;
                }
            };

            let mut writer: IndexWriter = match index_guard.writer(settings.compact_index_buffer) {
                Ok(w) => w,
                Err(e) => {
                    error!("Failed to create writer for {}: {}", key, e);
                    messages.push(format!("Failed to compact {}: writer error", key));
                    continue;
                }
            };

            drop(index_guard);
            if let Err(e) = writer.commit() {
                error!("Failed to commit before merge for {}: {}", key, e);
                messages.push(format!("Failed to merge index for {}", key));
                continue;
            }

            let read_guard = match index_lock.read() {
                Ok(guard) => guard,
                Err(e) => {
                    error!("Failed to acquire read lock for {}: {}", key, e);
                    messages.push(format!("Failed to read segments for {}", key));
                    continue;
                }
            };

            let reader = match read_guard.reader() {
                Ok(r) => r,
                Err(e) => {
                    error!("Failed to create reader for {}: {}", key, e);
                    messages.push(format!("Failed to read segments for {}", key));
                    continue;
                }
            };
            let segment_ids: Vec<SegmentId> = reader.searcher().segment_readers().iter().map(|r| r.segment_id()).collect();
            drop(reader);
            if segment_ids.len() > 1 {
                let merge_result = writer.merge(&segment_ids).await;
                if let Err(e) = merge_result {
                    error!("Failed to merge segments for {}: {}", key, e);
                    messages.push(format!("Failed to merge index for {}", key));
                } else {
                    if let Err(e) = writer.commit() {
                        error!("Failed to commit after merge for {}: {}", key, e);
                    }
                    messages.push(format!("Merged index for {}", key));
                }
            } else {
                messages.push(format!("No merge needed for {}", key));
            }
        }
    }
    HttpResponse::Ok().json(json!({"results": messages}))
}
