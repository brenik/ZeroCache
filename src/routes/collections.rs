use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_json::json;
use log::warn;
use crate::models::AppState;

pub async fn get_all_collections(
    app_data: web::Data<AppState>,
    _req: HttpRequest,
) -> impl Responder {
    let mut collections_list = Vec::new();
    let collections_lock = app_data.collections.read().unwrap();
    for (name, info) in collections_lock.iter() {
        let tree = app_data.db.open_tree(name).unwrap();
        let schema = info.index.read().unwrap().schema();
        let mut indexed_fields: Vec<String> = vec![];
        let mut primary_found = false;
        for (field, field_entry) in schema.fields() {
            let field_name = schema.get_field_name(field);
            if field_entry.is_stored() && !primary_found {
                indexed_fields.push(format!("{}(primary)", field_name));
                primary_found = true;
            } else if field_name != "text" {
                indexed_fields.push(field_name.to_string());
            }
        }
        if !primary_found {
            warn!("No primary (stored) field found in schema for collection {}", name);
        }
        collections_list.push(json!({
            "name": name,
            "count": tree.len(),
            "indexed": indexed_fields,
        }));
    }
    HttpResponse::Ok().json(json!({
        "collections": collections_list,
        "total": collections_list.len()
    }))
}