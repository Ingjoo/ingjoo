use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Multipart, Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, SET_COOKIE};
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::response::Response;
use axum::Extension;
use axum::Json;
use ingjoo_core::{
    AuthToken, LoginRequest, RegisterRequest, UpdatePreferences, UpdateProfileRequest, User, UserId, UserPreferences,
    UserPublic,
};
use serde::Deserialize;

use crate::auth::AuthProvider;
use crate::db::traits::IngjooStore;
use crate::middleware::error::AppError;
use crate::AppState;
use ingjoo_core::db::error::StoreError;

/// 构建 httpOnly 认证 cookie 的 Set-Cookie 头
fn auth_cookies(access_token: &str, refresh_token: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let secure = if std::env::var("INGJOO_COOKIE_SECURE").as_deref() == Ok("true") { "; Secure" } else { "" };
    let access_cookie =
        format!("access_token={}; HttpOnly; SameSite=Strict; Path=/api; Max-Age=3600{}", access_token, secure);
    let refresh_cookie = format!(
        "refresh_token={}; HttpOnly; SameSite=Strict; Path=/api/auth/refresh; Max-Age=604800{}",
        refresh_token, secure
    );
    headers.insert(SET_COOKIE, HeaderValue::from_str(&access_cookie).unwrap_or_else(|_| HeaderValue::from_static("")));
    headers.append(SET_COOKIE, HeaderValue::from_str(&refresh_cookie).unwrap_or_else(|_| HeaderValue::from_static("")));
    headers
}

/// 构建清除认证 cookie 的 Set-Cookie 头（Max-Age=0）
fn clear_cookies() -> HeaderMap {
    let mut headers = HeaderMap::new();
    let access_cookie = "access_token=; HttpOnly; SameSite=Strict; Path=/api; Max-Age=0";
    let refresh_cookie = "refresh_token=; HttpOnly; SameSite=Strict; Path=/api/auth/refresh; Max-Age=0";
    headers.insert(SET_COOKIE, HeaderValue::from_static(access_cookie));
    headers.append(SET_COOKIE, HeaderValue::from_static(refresh_cookie));
    headers
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, HeaderMap, Json<AuthToken>), AppError> {
    let hash = state.auth.hash_password(&req.password)?;
    let user = User {
        id: UserId::new(uuid::Uuid::new_v4().to_string()),
        email: req.email,
        name: req.name,
        password_hash: Some(hash),
        avatar_url: None,
        bio: None,
        role: "user".to_string(),
        oauth_provider: None,
        oauth_id: None,
        phone: None,
        created_at: String::new(),
        updated_at: String::new(),
    };
    let user = state.store.create_user(&user).await.map_err(|e| match e {
        StoreError::UniqueViolation { .. } => AppError::BadRequest("邮箱已注册".into()),
        other => AppError::from(other),
    })?;

    if let Some(user_group) = state.store.get_group_by_name("user").await? {
        let _ = state.store.add_user_to_group(&user.id, &user_group.id).await;
    }
    let groups = state.store.resolve_all_groups(&user.id).await.unwrap_or_else(|_| vec!["user".to_string()]);

    let access_token = state.auth.create_access_token(&user.id.0, &user.role, &groups, None)?;
    let refresh_token = state.auth.create_refresh_token();
    let token_hash = state.auth.refresh_token_hash(&refresh_token);
    let expires_at = state.auth.refresh_expires_at();
    state.store.create_refresh_token(&uuid::Uuid::new_v4().to_string(), &user.id, &token_hash, &expires_at).await?;
    let cookies = auth_cookies(&access_token, &refresh_token);
    Ok((StatusCode::CREATED, cookies, Json(AuthToken { access_token, refresh_token, user: UserPublic::from(&user) })))
}

async fn resolve_login_store(
    state: &AppState,
    database: Option<&str>,
) -> Result<Arc<dyn IngjooStore>, AppError> {
    match database {
        Some(db_name) if !db_name.is_empty() => {
            let (pool, dialect) = state
                .db_manager
                .get_pool(db_name)
                .await
                .map_err(|e| AppError::BadRequest(format!("数据库 '{}' 不可用: {}", db_name, e)))?;
            Ok(Arc::new(crate::db::Db::with_dialect(pool.as_ref().clone(), dialect)))
        }
        _ => Ok(state.store.clone()),
    }
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<(HeaderMap, Json<AuthToken>), AppError> {
    let login_store = resolve_login_store(&state, req.database.as_deref()).await?;

    let user = login_store
        .get_user_by_email(&req.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("邮箱或密码错误".into()))?;
    let hash = user.password_hash.as_ref().ok_or_else(|| AppError::Unauthorized("邮箱或密码错误".into()))?;
    if !state.auth.verify_password(&req.password, hash)? {
        return Err(AppError::Unauthorized("邮箱或密码错误".into()));
    }
    let groups = login_store.resolve_all_groups(&user.id).await.unwrap_or_else(|_| vec![user.role.clone()]);
    let access_token = state.auth.create_access_token(&user.id.0, &user.role, &groups, req.database.as_deref())?;
    let refresh_token = state.auth.create_refresh_token();
    let token_hash = state.auth.refresh_token_hash(&refresh_token);
    let expires_at = state.auth.refresh_expires_at();
    login_store.create_refresh_token(&uuid::Uuid::new_v4().to_string(), &user.id, &token_hash, &expires_at).await?;
    let cookies = auth_cookies(&access_token, &refresh_token);
    Ok((cookies, Json(AuthToken { access_token, refresh_token, user: UserPublic::from(&user) })))
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshRequest>,
) -> Result<(HeaderMap, Json<AuthToken>), AppError> {
    let token_hash = state.auth.refresh_token_hash(&body.refresh_token);
    let (user_id_str, _expires_at) = state
        .store
        .get_refresh_token(&token_hash)
        .await?
        .ok_or_else(|| AppError::Unauthorized("无效的刷新令牌".into()))?;
    let user_id = UserId::new(user_id_str);
    let user =
        state.store.get_user_by_id(&user_id).await?.ok_or_else(|| AppError::Unauthorized("用户不存在".into()))?;
    state.store.delete_refresh_token(&token_hash).await?;
    let groups = state.store.resolve_all_groups(&user.id).await.unwrap_or_else(|_| vec![user.role.clone()]);
    let access_token = state.auth.create_access_token(&user.id.0, &user.role, &groups, None)?;
    let new_refresh = state.auth.create_refresh_token();
    let new_hash = state.auth.refresh_token_hash(&new_refresh);
    let expires_at = state.auth.refresh_expires_at();
    state.store.create_refresh_token(&uuid::Uuid::new_v4().to_string(), &user.id, &new_hash, &expires_at).await?;
    let cookies = auth_cookies(&access_token, &new_refresh);
    Ok((cookies, Json(AuthToken { access_token, refresh_token: new_refresh, user: UserPublic::from(&user) })))
}

pub async fn get_profile(
    Extension(current_user): Extension<crate::extractors::CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<UserPublic>, AppError> {
    let user_id = UserId::new(current_user.user_id);
    let user = state.store.get_user_by_id(&user_id).await?.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    Ok(Json(UserPublic::from(&user)))
}

pub async fn update_profile(
    Extension(current_user): Extension<crate::extractors::CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<Json<UserPublic>, AppError> {
    let user_id = UserId::new(current_user.user_id);
    let user = state
        .store
        .update_user(&user_id, req.name.as_deref(), req.avatar_url.as_deref(), req.bio.as_deref())
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    Ok(Json(UserPublic::from(&user)))
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

pub async fn change_password(
    Extension(current_user): Extension<crate::extractors::CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<StatusCode, AppError> {
    let user_id = UserId::new(current_user.user_id);
    let user = state.store.get_user_by_id(&user_id).await?.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    let hash = user.password_hash.as_ref().ok_or_else(|| AppError::Unauthorized("当前密码错误".into()))?;
    if !state.auth.verify_password(&req.current_password, hash)? {
        return Err(AppError::Unauthorized("当前密码错误".into()));
    }
    let new_hash = state.auth.hash_password(&req.new_password)?;
    state.store.update_user_password(&user_id, &new_hash).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_preferences(
    Extension(current_user): Extension<crate::extractors::CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<UserPreferences>, AppError> {
    let user_id = UserId::new(current_user.user_id);
    let prefs = state.store.get_user_preferences(&user_id).await?;
    Ok(Json(prefs))
}

pub async fn update_preferences(
    Extension(current_user): Extension<crate::extractors::CurrentUser>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdatePreferences>,
) -> Result<Json<UserPreferences>, AppError> {
    let user_id = UserId::new(current_user.user_id);
    let prefs = state.store.upsert_user_preferences(&user_id, &req).await?;
    Ok(Json(prefs))
}

pub async fn get_me(
    Extension(current_user): Extension<crate::extractors::CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<UserPublic>, AppError> {
    let user_id = UserId::new(current_user.user_id);
    let user = state.store.get_user_by_id(&user_id).await?.ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    Ok(Json(UserPublic::from(&user)))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RefreshRequest>,
) -> Result<(StatusCode, HeaderMap), AppError> {
    let token_hash = state.auth.refresh_token_hash(&req.refresh_token);
    let exists = state.store.get_refresh_token(&token_hash).await?.is_some();
    if !exists {
        return Err(AppError::Unauthorized("无效的刷新令牌".into()));
    }
    state.store.delete_refresh_token(&token_hash).await?;
    Ok((StatusCode::NO_CONTENT, clear_cookies()))
}

/// mime 类型 → 文件扩展名
fn mime_to_ext(mime: &str) -> Option<&'static str> {
    match mime {
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

/// 上传头像 — multipart 表单，限制 2MB，仅接受图片
pub async fn upload_avatar(
    Extension(current_user): Extension<crate::extractors::CurrentUser>,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<UserPublic>, AppError> {
    let field = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(format!("读取上传数据失败: {}", e)))?
        .ok_or_else(|| AppError::BadRequest("未找到上传文件".into()))?;

    let content_type = field.content_type().unwrap_or("").to_string();
    if !content_type.starts_with("image/") {
        return Err(AppError::BadRequest("仅支持图片文件".into()));
    }

    let ext = mime_to_ext(&content_type).ok_or_else(|| AppError::BadRequest("不支持的图片格式".into()))?;

    let data = field.bytes().await.map_err(|e| AppError::BadRequest(format!("读取上传数据失败: {}", e)))?;

    if data.len() > 2_097_152 {
        return Err(AppError::BadRequest("文件大小不能超过 2MB".into()));
    }

    let filename = format!("avatars/{}_{}.{}", current_user.user_id, uuid::Uuid::new_v4(), ext);
    state.file_storage.save(&filename, &data).await?;

    let avatar_url = format!("/api/avatars/{}", filename.trim_start_matches("avatars/"));
    let user_id = UserId::new(current_user.user_id);
    let user = state
        .store
        .update_user(&user_id, None, Some(&avatar_url), None)
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".into()))?;

    Ok(Json(UserPublic::from(&user)))
}

pub async fn serve_avatar(
    State(state): State<Arc<AppState>>,
    Path(filename): Path<String>,
) -> Result<Response, AppError> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') || filename.contains('\0') {
        return Err(AppError::BadRequest("非法文件名".into()));
    }

    let path = format!("avatars/{}", filename);
    let data = state.file_storage.load(&path).await.map_err(|_| AppError::NotFound("头像文件不存在".into()))?;

    let ext = std::path::Path::new(&filename).extension().and_then(|e| e.to_str()).unwrap_or("");
    let content_type = match ext {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header(CACHE_CONTROL, "public, max-age=86400")
        .body(Body::from(data))
        .unwrap_or_else(|_| {
            Response::builder().status(StatusCode::INTERNAL_SERVER_ERROR).body(Body::from("服务器内部错误")).unwrap()
        }))
}

/// 忘记密码请求
#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

/// 忘记密码 — noop 实现，始终返回成功防止邮箱枚举
///
/// 当 email provider 激活后，此处应发送重置链接。
/// 当前使用 NoopEmailProvider，仅记录日志。
pub async fn forgot_password(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<ForgotPasswordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    tracing::info!("忘记密码请求: email={}", req.email);

    #[cfg(feature = "email")]
    {
        let _ = _state.email.send(&req.email, "密码重置", "您请求了密码重置。").await;
    }

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": "如果该邮箱已注册，重置链接已发送"
    })))
}
