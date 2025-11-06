use actix_web::dev::{Service, ServiceRequest, ServiceResponse};
use actix_web::{web, HttpRequest, HttpResponse};
use log::warn;
use serde_json::json;
use crate::models::AppState;

pub fn get_client_ip(req: &HttpRequest) -> String {
    req.peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

pub fn is_ip_allowed(ip: &str, allowed_ips: &[String]) -> bool {
    for allowed in allowed_ips {
        if allowed == ip {
            return true;
        }
        if allowed.contains('*') {
            let pattern = allowed.replace('*', "");
            if ip.starts_with(&pattern) || ip.ends_with(&pattern) {
                return true;
            }
        }
    }
    false
}

pub async fn ip_check_middleware<S>(
    req: ServiceRequest,
    srv: S,
) -> Result<ServiceResponse, actix_web::Error>
where
    S: Service<ServiceRequest, Response = ServiceResponse, Error = actix_web::Error> + 'static,
{
    let method = req.method().clone();
    let path = req.path().to_string();
    let client_ip = get_client_ip(req.request());
    let app_data_clone = req.app_data::<web::Data<AppState>>().cloned();
    if matches!(method, actix_web::http::Method::POST | actix_web::http::Method::PUT | actix_web::http::Method::DELETE)
        || matches!(path.as_str(),
            "/purge" |
            "/compact" |
            "/settings" |
            "/status" |
            "/trees"
        ) {
        if let Some(app_data) = app_data_clone {
            let settings = app_data.settings.read().unwrap();
            if !is_ip_allowed(&client_ip, &settings.allowed_ips) {
                warn!("Access denied for IP: {}", client_ip);
                let response = HttpResponse::Forbidden().json(json!({
                    "error": "Access denied",
                    "your_ip": client_ip
                }));
                let (http_req, _) = req.into_parts();
                return Ok(ServiceResponse::new(http_req, response));
            }
        }
    }
    srv.call(req).await
}