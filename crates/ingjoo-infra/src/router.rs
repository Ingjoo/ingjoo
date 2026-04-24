use std::sync::Arc;

use axum::middleware;
use axum::routing::{any, get, post, put};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::middleware::permission::require_admin;
use crate::middleware::ratelimit::{rate_limit_middleware, RateLimiter};
use crate::middleware::security::auth_middleware;
use crate::AppState;

pub fn base_router(state: Arc<AppState>) -> Router {
    let cors = match std::env::var("CORS_ORIGIN") {
        Ok(origin) if origin != "*" => {
            let hv = origin.parse().unwrap_or_else(|_| axum::http::HeaderValue::from_static("*"));
            CorsLayer::new().allow_origin(hv).allow_methods(Any).allow_headers(Any)
        }
        _ => CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any),
    };

    let limiter = Arc::new(RateLimiter::new(10, 1.0));

    let public_routes = Router::new()
        .route("/api/health", get(handlers::health::health_check))
        .route("/api/auth/register", post(handlers::auth::register))
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/refresh", post(handlers::auth::refresh))
        .route("/ws", any(handlers::ws::ws_handler))
        .route_layer(middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(limiter));

    let protected_routes = Router::new()
        .route(
            "/api/auth/profile",
            get(handlers::auth::get_profile).put(handlers::auth::update_profile),
        )
        .route("/api/settings", get(handlers::settings::list_settings))
        .route("/api/settings/{key}", put(handlers::settings::set_setting))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let admin_routes = Router::new()
        .route("/api/groups", get(handlers::groups::list_groups).post(handlers::groups::create_group))
        .route("/api/groups/{id}", get(handlers::groups::get_group).put(handlers::groups::update_group).delete(handlers::groups::delete_group))
        .route("/api/groups/{id}/implied", get(handlers::groups::get_implied_groups))
        .route("/api/users/{user_id}/groups", get(handlers::groups::get_user_groups).put(handlers::groups::set_user_groups))
        .route("/api/access", get(handlers::permission::list_model_accesses).post(handlers::permission::create_model_access))
        .route("/api/access/{id}", get(handlers::permission::get_model_access).put(handlers::permission::update_model_access).delete(handlers::permission::delete_model_access))
        .route("/api/rules", get(handlers::permission::list_record_rules).post(handlers::permission::create_record_rule))
        .route("/api/rules/{id}", get(handlers::permission::get_record_rule).put(handlers::permission::update_record_rule).delete(handlers::permission::delete_record_rule))
        .route("/api/schedules", get(handlers::schedule::list_schedules).post(handlers::schedule::create_schedule))
        .route("/api/schedules/{id}", get(handlers::schedule::get_schedule).put(handlers::schedule::update_schedule).delete(handlers::schedule::delete_schedule))
        // auth_middleware 在外层（先执行），require_admin 在内层（后执行）
        // 请求流: auth_middleware → require_admin → handler
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let crud_routes = Router::new()
        .route("/api/models", get(handlers::crud::crud_models))
        .route("/api/data/{model}", get(handlers::crud::crud_list).post(handlers::crud::crud_create))
        .route("/api/data/{model}/{id}", get(handlers::crud::crud_read).put(handlers::crud::crud_update).delete(handlers::crud::crud_delete))
        .route("/api/data/{model}/ensure", post(handlers::crud::crud_ensure_table))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // 元数据路由 — 认证用户可读菜单树和动作详情，管理员可管理
    let metadata_read_routes = Router::new()
        .route("/api/menus", get(handlers::menu::list_menus))
        .route("/api/actions/{id}", get(handlers::action::get_action))
        .route("/api/views", get(handlers::view::list_views))
        .route("/api/views/{id}", get(handlers::view::get_view))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let metadata_admin_routes = Router::new()
        .route("/api/menus", post(handlers::menu::create_menu))
        .route("/api/menus/{id}", put(handlers::menu::update_menu).delete(handlers::menu::delete_menu))
        .route("/api/actions", get(handlers::action::list_actions).post(handlers::action::create_action))
        .route("/api/actions/{id}", put(handlers::action::update_action).delete(handlers::action::delete_action))
        .route("/api/views", post(handlers::view::create_view))
        .route("/api/views/{id}", put(handlers::view::update_view).delete(handlers::view::delete_view))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(admin_routes)
        .merge(crud_routes)
        .merge(metadata_read_routes)
        .merge(metadata_admin_routes)
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
