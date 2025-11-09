use serde_json::Value;
use tantivy::schema::{Schema, TEXT, STORED, NumericOptions};

pub fn get_object_id_from_json(item: &Value, key_field: &str) -> Option<String> {
    item.get(key_field)
        .and_then(|id_val| id_val.as_u64().map(|id| id.to_string()))
        .or_else(|| item.get(key_field).and_then(|id_val| id_val.as_str().map(|s| s.to_string())))
}

pub fn strip_type_suffix(field_spec: &str) -> &str {
    field_spec.split_once(':')
        .map(|(name, _)| name)
        .unwrap_or(field_spec)
}

pub fn create_tantivy_schema(primary: &str, index_fields: &[String]) -> Schema {
    let mut schema_builder = Schema::builder();

    schema_builder.add_text_field(primary, TEXT | STORED);

    for field_spec in index_fields {
        if let Some((field_name, field_type)) = field_spec.split_once(':') {
            match field_type {
                "u64" => {
                    let opts = NumericOptions::default()
                        .set_indexed()
                        .set_fast();
                    schema_builder.add_u64_field(field_name, opts);
                }
                "i64" => {
                    let opts = NumericOptions::default()
                        .set_indexed()
                        .set_fast();
                    schema_builder.add_i64_field(field_name, opts);
                }
                "f64" => {
                    let opts = NumericOptions::default()
                        .set_indexed()
                        .set_fast();
                    schema_builder.add_f64_field(field_name, opts);
                }
                _ => {
                    schema_builder.add_text_field(field_name, TEXT);
                }
            }
        } else {
            schema_builder.add_text_field(field_spec, TEXT);
        }
    }

    schema_builder.add_text_field("text", TEXT);

    schema_builder.build()
}