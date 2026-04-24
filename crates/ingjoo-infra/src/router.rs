use axum::http::HeaderValue;
use axum::Router;
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::TraceLayer;

pub fn base_router() -> Router {
    let cors = match std::env::var("CORS_ORIGIN") {
        Ok(origin) if origin != "*" => {
            let hv: HeaderValue = origin.parse().unwrap_or_else(|_| HeaderValue::from_static("*"));
            CorsLayer::new().allow_origin(hv).allow_methods(Any).allow_headers(Any)
        }
        _ => CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any),
    };

    Router::new()
        .layer(cors)
        .layer(TraceLayer::new_for_http())
}
