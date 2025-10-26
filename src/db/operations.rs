use serde_json::Value;
use tantivy::schema::{Schema, TEXT, STORED};

pub fn get_object_id_from_json(item: &Value, key_field: &str) -> Option<String> {
    item.get(key_field)
        .and_then(|id_val| id_val.as_u64().map(|id| id.to_string()))
        .or_else(|| item.get(key_field).and_then(|id_val| id_val.as_str().map(|s| s.to_string())))
}

pub fn create_tantivy_schema(primary: &str, index_fields: &[String]) -> Schema {
    let mut schema_builder = Schema::builder();
    schema_builder.add_text_field(primary, TEXT | STORED);
    for field in index_fields {
        schema_builder.add_text_field(field, TEXT);
    }
    schema_builder.add_text_field("text", TEXT);
    schema_builder.build()
}