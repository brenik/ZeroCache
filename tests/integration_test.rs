use reqwest;
use serde_json::{json, Value};

const BASE_URL: &str = "http://127.0.0.1:8080";

async fn create_test_collection(collection_name: &str) {
    let client = reqwest::Client::new();
    
    let ndjson_data = r#"{"objectID":"1","name":"Wireless Mouse","description":"Ergonomic wireless mouse with adjustable DPI and long battery life.","price":29.99,"category":"Electronics"}
{"objectID":"2","name":"Mechanical Keyboard","description":"RGB backlit mechanical keyboard with cherry switches for gaming and typing.","price":89.99,"category":"Electronics"}
{"objectID":"3","name":"Running Shoes","description":"Lightweight running shoes with cushioning for marathon training.","price":59.99,"category":"Sports"}
{"objectID":"4","name":"Coffee Maker","description":"Automatic coffee maker with programmable timer and 12-cup capacity.","price":49.99,"category":"Home Appliances"}
{"objectID":"5","name":"Bluetooth Speaker","description":"Portable Bluetooth speaker with waterproof design and 10-hour battery.","price":39.99,"category":"Electronics"}
{"objectID":"6","name":"Yoga Mat","description":"Non-slip yoga mat made from eco-friendly materials, 6mm thick.","price":24.99,"category":"Sports"}
{"objectID":"7","name":"Electric Kettle","description":"Fast-boiling electric kettle with auto shut-off and 1.7L capacity.","price":19.99,"category":"Home Appliances"}
{"objectID":"8","name":"Gaming Headset","description":"Over-ear gaming headset with noise cancellation and surround sound.","price":69.99,"category":"Electronics"}
{"objectID":"9","name":"Dumbbell Set","description":"Adjustable dumbbell set from 5 to 50 lbs for home workouts.","price":99.99,"category":"Sports"}
{"objectID":"10","name":"Blender","description":"High-speed blender with multiple settings for smoothies and soups.","price":34.99,"category":"Home Appliances"}"#;
    
    client.post(format!("{}/data/{}", BASE_URL, collection_name))
        .header("Content-Type", "application/x-ndjson")
        .header("X-Upsert-Field", "objectID,name,description,category,price")
        .body(ndjson_data)
        .send()
        .await
        .expect("Failed to create test collection");
}

#[tokio::test]
async fn test_status() {
    let client = reqwest::Client::new();
    let response = client.get(format!("{}/status", BASE_URL))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn test_settings_get() {
    let client = reqwest::Client::new();
    let response = client.get(format!("{}/settings", BASE_URL))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(body["port"].is_number());
    assert!(body["allowed_ips"].is_array());
}

#[tokio::test]
async fn test_settings_update() {
    let client = reqwest::Client::new();
    
    let update_data = json!({
        "compact_index_buffer": 45000000
    });
    
    let response = client.put(format!("{}/settings", BASE_URL))
        .json(&update_data)
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    
    let get_response = client.get(format!("{}/settings", BASE_URL))
        .send()
        .await
        .expect("Failed to get settings");
    
    let body: Value = get_response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["compact_index_buffer"]["bytes"], 45000000);
}

#[tokio::test]
async fn test_upsert_data() {
    let collection = "test_upsert";
    create_test_collection(collection).await;
    
    let client = reqwest::Client::new();
    let response = client.get(format!("{}/data/{}", BASE_URL, collection))
        .send()
        .await
        .expect("Failed to get collection");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["total"], 10);
    
    client.delete(format!("{}/data/{}", BASE_URL, collection))
        .header("X-Confirm-Purge", "true")
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_all_items() {
    let collection = "test_get_all";
    create_test_collection(collection).await;
    
    let client = reqwest::Client::new();
    let response = client.get(format!("{}/data/{}", BASE_URL, collection))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(body[collection].is_array());
    assert_eq!(body["total"].as_u64().unwrap(), 10);
    
    client.delete(format!("{}/data/{}", BASE_URL, collection))
        .header("X-Confirm-Purge", "true")
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_with_query() {
    let collection = "test_query";
    create_test_collection(collection).await;
    
    let client = reqwest::Client::new();
    let response = client.get(format!("{}/data/{}?q=Gaming", BASE_URL, collection))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(body[collection].as_array().unwrap().len() >= 1);
    
    client.delete(format!("{}/data/{}", BASE_URL, collection))
        .header("X-Confirm-Purge", "true")
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_with_category_filter() {
    let collection = "test_category";
    create_test_collection(collection).await;
    
    let client = reqwest::Client::new();
    let response = client.get(format!("{}/data/{}?category=Electronics", BASE_URL, collection))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(body[collection].as_array().unwrap().len() >= 3);
    
    client.delete(format!("{}/data/{}", BASE_URL, collection))
        .header("X-Confirm-Purge", "true")
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_with_price_filter() {
    let collection = "test_price";
    create_test_collection(collection).await;
    
    let client = reqwest::Client::new();
    let response = client.get(format!("{}/data/{}?price=99.99", BASE_URL, collection))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(body[collection].as_array().unwrap().len() >= 1);
    
    client.delete(format!("{}/data/{}", BASE_URL, collection))
        .header("X-Confirm-Purge", "true")
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_with_pagination() {
    let collection = "test_pagination";
    create_test_collection(collection).await;
    
    let client = reqwest::Client::new();
    let response = client.get(format!("{}/data/{}?limit=3&offset=2", BASE_URL, collection))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body[collection].as_array().unwrap().len(), 3);
    assert_eq!(body["total"], 3);
    
    client.delete(format!("{}/data/{}", BASE_URL, collection))
        .header("X-Confirm-Purge", "true")
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_with_sorting() {
    let collection = "test_sorting";
    create_test_collection(collection).await;
    
    let client = reqwest::Client::new();
    let response = client.get(format!("{}/data/{}?sort_by=price&sort_order=desc", BASE_URL, collection))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    let items = body[collection].as_array().unwrap();
    assert_eq!(items[0]["price"], 99.99);
    
    client.delete(format!("{}/data/{}", BASE_URL, collection))
        .header("X-Confirm-Purge", "true")
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_get_trees() {
    let client = reqwest::Client::new();
    
    let response = client.get(format!("{}/trees", BASE_URL))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(body["collections"].is_array());
    assert!(body["total"].is_number());
}

#[tokio::test]
async fn test_delete_by_primary_key() {
    let collection = "test_delete_pk";
    create_test_collection(collection).await;
    
    let client = reqwest::Client::new();
    
    let response = client.delete(format!("{}/data/{}?objectID=5", BASE_URL, collection))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert_eq!(body["deleted"], 1);
    assert_eq!(body["id"], "5");
    
    let check = client.get(format!("{}/data/{}", BASE_URL, collection))
        .send()
        .await
        .expect("Failed to get collection");
    let check_body: Value = check.json().await.expect("Failed to parse JSON");
    assert_eq!(check_body["total"], 9);
    
    client.delete(format!("{}/data/{}", BASE_URL, collection))
        .header("X-Confirm-Purge", "true")
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_delete_collection_without_confirm() {
    let collection = "test_del_no_confirm";
    create_test_collection(collection).await;
    
    let client = reqwest::Client::new();
    let response = client.delete(format!("{}/data/{}", BASE_URL, collection))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_ne!(response.status(), 200);
    
    client.delete(format!("{}/data/{}", BASE_URL, collection))
        .header("X-Confirm-Purge", "true")
        .send()
        .await
        .ok();
}

#[tokio::test]
async fn test_delete_collection_with_confirm() {
    let collection = "test_del_confirm";
    create_test_collection(collection).await;
    
    let client = reqwest::Client::new();
    let response = client.delete(format!("{}/data/{}", BASE_URL, collection))
        .header("X-Confirm-Purge", "true")
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), 200);
    let body: Value = response.json().await.expect("Failed to parse JSON");
    assert!(body["message"].as_str().unwrap().to_lowercase().contains("deleted"));
}
