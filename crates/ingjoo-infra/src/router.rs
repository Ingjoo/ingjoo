use std::sync::Arc;

use axum::http::Method;
use axum::middleware;
use axum::routing::{any, delete, get, post, put};
use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::handlers;
#[cfg(feature = "multi-db")]
use crate::middleware::admin_basic_auth;
use crate::middleware::database_selector::database_selector_middleware;
use crate::middleware::headers::security_headers_middleware;
use crate::middleware::permission::require_admin;
use crate::middleware::ratelimit::{rate_limit_middleware, RateLimiter};
use crate::middleware::security::auth_middleware;
use crate::AppState;

/// 构建应用根路由，挂载公开/认证/管理员/CRUD/元数据全部端点
pub fn base_router(state: Arc<AppState>) -> Router {
    let cors = match std::env::var("CORS_ORIGIN") {
        Ok(origin) if !origin.is_empty() && origin != "*" => {
            let hv = origin.parse().unwrap_or_else(|_| axum::http::HeaderValue::from_static("*"));
            CorsLayer::new()
                .allow_origin(hv)
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                .allow_headers(Any)
        }
        Ok(origin) if origin == "*" => {
            tracing::warn!("CORS_ORIGIN=* 允许所有来源，不建议在生产环境使用");
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                .allow_headers(Any)
        }
        _ => CorsLayer::new()
            .allow_origin(["http://localhost:3001".parse().unwrap(), "http://127.0.0.1:3001".parse().unwrap()])
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
            .allow_headers(Any),
    };

    // 分层限流：公开路由用配置值，认证/管理员/CRUD 路由用更高限额
    let public_limiter =
        Arc::new(RateLimiter::new(state.rate_limit.max_tokens as u32, state.rate_limit.refill_per_sec));
    let protected_limiter = Arc::new(RateLimiter::new(30, 5.0));
    let admin_limiter = Arc::new(RateLimiter::new(60, 10.0));
    let crud_limiter = Arc::new(RateLimiter::new(30, 5.0));

    let public_routes = Router::new()
        .route("/api/health", get(handlers::health::health_check))
        .route("/healthz", get(handlers::health::liveness_check))
        .route("/readyz", get(handlers::health::readiness_check))
        .route("/api/auth/register", post(handlers::auth::register))
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/logout", post(handlers::auth::logout))
        .route("/api/auth/refresh", post(handlers::auth::refresh))
        .route("/api/auth/forgot-password", post(handlers::auth::forgot_password))
        .route("/api/avatars/{filename}", get(handlers::auth::serve_avatar))
        .route("/ws", any(handlers::ws::ws_handler))
        .route_layer(middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(public_limiter.clone()));

    #[cfg(feature = "captcha")]
    let public_routes = public_routes
        .route("/api/captcha", get(handlers::captcha::generate_captcha).post(handlers::captcha::verify_captcha));

    let protected_routes = Router::new()
        .route("/api/auth/me", get(handlers::auth::get_me))
        .route("/api/auth/profile", get(handlers::auth::get_profile).put(handlers::auth::update_profile))
        .route("/api/auth/avatar", post(handlers::auth::upload_avatar))
        .route("/api/auth/change-password", post(handlers::auth::change_password))
        .route("/api/auth/preferences", get(handlers::auth::get_preferences).put(handlers::auth::update_preferences))
        .route("/api/settings", get(handlers::settings::list_settings))
        .route("/api/settings/{key}", put(handlers::settings::set_setting))
        .route("/api/settings/definitions", get(handlers::settings::list_settings_definitions))
        .route("/api/dashboard/stats", get(handlers::dashboard::get_stats))
        .route("/api/users/search", get(handlers::users::search_users))
        .route("/api/notifications", get(handlers::notification::list_notifications))
        .route("/api/notifications/unread-count", get(handlers::notification::get_unread_count))
        .route("/api/notifications/mark-all-read", post(handlers::notification::mark_all_read))
        .route("/api/notifications/{id}/mark-read", post(handlers::notification::mark_read))
        .route("/api/events", get(handlers::events::sse_events))
        .route("/api/search", post(handlers::search::global_search))
        .route_layer(middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(protected_limiter.clone()))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let admin_routes = Router::new()
        .route("/api/groups", get(handlers::groups::list_groups).post(handlers::groups::create_group))
        .route(
            "/api/groups/{id}",
            get(handlers::groups::get_group).put(handlers::groups::update_group).delete(handlers::groups::delete_group),
        )
        .route("/api/groups/{id}/implied", get(handlers::groups::get_implied_groups))
        .route(
            "/api/users/{user_id}/groups",
            get(handlers::groups::get_user_groups).put(handlers::groups::set_user_groups),
        )
        .route(
            "/api/access",
            get(handlers::permission::list_model_accesses).post(handlers::permission::create_model_access),
        )
        .route(
            "/api/access/{id}",
            get(handlers::permission::get_model_access)
                .put(handlers::permission::update_model_access)
                .delete(handlers::permission::delete_model_access),
        )
        .route(
            "/api/rules",
            get(handlers::permission::list_record_rules).post(handlers::permission::create_record_rule),
        )
        .route(
            "/api/rules/{id}",
            get(handlers::permission::get_record_rule)
                .put(handlers::permission::update_record_rule)
                .delete(handlers::permission::delete_record_rule),
        )
        .route("/api/schedules", get(handlers::schedule::list_schedules).post(handlers::schedule::create_schedule))
        .route(
            "/api/schedules/{id}",
            get(handlers::schedule::get_schedule)
                .put(handlers::schedule::update_schedule)
                .delete(handlers::schedule::delete_schedule),
        )
        .route("/api/databases", get(handlers::database::list_databases).post(handlers::database::create_database))
        .route("/api/databases/{name}/status", get(handlers::database::get_database_status))
        .route("/api/databases/{name}", delete(handlers::database::delete_database))
        .route("/api/admin/plugins", get(handlers::plugin::list_plugins))
        .route("/api/admin/plugins/load", post(handlers::plugin::load_all_plugins))
        .route("/api/admin/plugins/{name}/unload", post(handlers::plugin::unload_plugin))
        .route("/api/admin/plugins/{name}/reload", post(handlers::plugin::reload_plugin))
        .route("/api/modules", get(handlers::modules::list_modules).post(handlers::modules::install_module))
        .route(
            "/api/modules/{name}",
            delete(handlers::modules::uninstall_module).put(handlers::modules::upgrade_module),
        )
        .route("/api/admin/audit-log", get(handlers::users::get_audit_log))
        .route("/api/admin/search/rebuild/{model}", post(handlers::search::rebuild_index))
        .route_layer(middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(admin_limiter.clone()))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let crud_routes = Router::new()
        .route("/api/models", get(handlers::crud::crud_models))
        .route("/api/data/{model}", get(handlers::crud::crud_list).post(handlers::crud::crud_create))
        .route(
            "/api/data/{model}/{id}",
            get(handlers::crud::crud_read).put(handlers::crud::crud_update).delete(handlers::crud::crud_delete),
        )
        .route(
            "/api/data/{model}/{id}/attachments",
            post(handlers::attachment::upload_attachment).get(handlers::attachment::list_attachments),
        )
        .route(
            "/api/data/{model}/{id}/attachments/{aid}",
            get(handlers::attachment::download_attachment).delete(handlers::attachment::delete_attachment),
        )
        .route("/api/data/{model}/search", get(handlers::crud::crud_search))
        .route("/api/data/{model}/ensure", post(handlers::crud::crud_ensure_table))
        .route_layer(middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(crud_limiter))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let metadata_read_routes = Router::new()
        .route("/api/menus", get(handlers::menu::list_menus))
        .route("/api/actions/{id}", get(handlers::action::get_action))
        .route("/api/views", get(handlers::view::list_views))
        .route("/api/views/{id}", get(handlers::view::get_view))
        .route_layer(middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(protected_limiter.clone()))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    let metadata_admin_routes = Router::new()
        .route("/api/menus", post(handlers::menu::create_menu))
        .route("/api/menus/{id}", put(handlers::menu::update_menu).delete(handlers::menu::delete_menu))
        .route("/api/actions", get(handlers::action::list_actions).post(handlers::action::create_action))
        .route("/api/actions/{id}", put(handlers::action::update_action).delete(handlers::action::delete_action))
        .route("/api/views", post(handlers::view::create_view))
        .route("/api/views/{id}", put(handlers::view::update_view).delete(handlers::view::delete_view))
        .route_layer(middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(admin_limiter.clone()))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_admin))
        .route_layer(middleware::from_fn_with_state(state.clone(), auth_middleware));

    // 多数据库管理 API（Basic Auth 保护）
    #[cfg(feature = "multi-db")]
    let db_mgmt_public = Router::new()
        .route("/api/database/list", get(handlers::database::public_list_databases))
        .route_layer(middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(public_limiter.clone()));

    #[cfg(feature = "multi-db")]
    let db_mgmt_protected = Router::new()
        .route("/api/database/create", post(handlers::database::public_create_database))
        .route("/api/database/{name}", delete(handlers::database::public_delete_database))
        .route("/api/database/{name}/info", get(handlers::database::public_database_info))
        .route("/api/database/{name}/backup", post(handlers::database::public_backup_database))
        .route("/api/database/{name}/restore", post(handlers::database::public_restore_database))
        .route("/api/database/backup/upload", post(handlers::database::public_upload_backup))
        .route("/api/database/backup/list", get(handlers::database::public_list_backups))
        .route_layer(middleware::from_fn(rate_limit_middleware))
        .layer(axum::Extension(admin_limiter.clone()))
        .route_layer(middleware::from_fn_with_state(state.clone(), admin_basic_auth::require_admin_basic_auth));

    #[allow(unused_mut)]
    let mut app = Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(admin_routes)
        .merge(crud_routes)
        .merge(metadata_read_routes)
        .merge(metadata_admin_routes);

    #[cfg(feature = "multi-db")]
    {
        app = app.merge(db_mgmt_public).merge(db_mgmt_protected);
    }

    app.layer(middleware::from_fn_with_state(state.clone(), database_selector_middleware))
        .layer(middleware::from_fn(security_headers_middleware))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
