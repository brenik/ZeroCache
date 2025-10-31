mod models;
mod config;
mod middleware;
mod utils;
mod db;
mod routes;

use models::AppState;
use config::{init_logger, load_settings};
use middleware::ip_check_middleware;
use db::load_existing_collections;
use routes::{
    set_item,
    get_item,
    delete_item,
    delete_all,
    compact,
    get_all_collections,
    get_settings,
    set_settings,
    get_status
};

use actix_web::{web, App,  HttpServer, middleware::from_fn};
use std::sync::{RwLock, Arc};
use actix_governor::{Governor, GovernorConfigBuilder};
use actix_governor::PeerIpKeyExtractor;
use std::sync::atomic::AtomicU64;
use std::time::Instant;



#[actix_web::main]
async fn main() -> std::io::Result<()> {
    init_logger();

    let settings = load_settings();
    let db = sled::open(&settings.data_path).expect("Failed to open Sled DB");
    let collections = load_existing_collections(&settings.index_path);

    let app_state = web::Data::new(AppState {
        db,
        collections: RwLock::new(collections),
        settings: RwLock::new(settings),
        start_time: Instant::now(),
        request_counter: Arc::new(AtomicU64::new(0)),
    });
    let bind_address = {
        let settings = app_state.settings.read().unwrap();
        format!("127.0.0.1:{}", settings.port)
    };
    let governor_conf = {
        let settings = app_state.settings.read().unwrap();
        GovernorConfigBuilder::default()
            .period(std::time::Duration::from_secs_f64(1.0 / settings.rate_limit_per_second as f64))
            .burst_size(settings.rate_limit_per_second)
            .key_extractor(PeerIpKeyExtractor)
            .finish()
            .unwrap()
    };
    let max_payload_size = {
        let settings = app_state.settings.read().unwrap();
        settings.payload_limit
    };
    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .app_data(web::PayloadConfig::new(max_payload_size))
            .wrap(from_fn(ip_check_middleware))
            .service(
                web::resource("/purge")
                    .route(web::delete().to(delete_all)),
            )
            .service(
                web::resource("/compact")
                    .route(web::delete().to(compact)),
            )
            .service(
                web::resource("/data/{key}")
                    .route(web::post().to(set_item))
                    .route(web::get().to(get_item).wrap(Governor::new(&governor_conf)))
                    .route(web::delete().to(delete_item)),
            )
            .service(
                web::resource("/trees")
                    .route(web::get().to(get_all_collections).wrap(Governor::new(&governor_conf))),
            )
            .service(
                web::resource("/settings")
                    .route(web::put().to(set_settings))
                    .route(web::get().to(get_settings).wrap(Governor::new(&governor_conf))),
            )
            .service(
                web::resource("/status")
                    .route(web::get().to(get_status).wrap(Governor::new(&governor_conf))),
            )
    })
        .bind(bind_address)?
        .run()
        .await
}