use std::sync::Arc;

use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use ingjoo_core::{
    AuthToken, LoginRequest, RegisterRequest, UpdatePreferences, UpdateProfileRequest, User, UserId, UserPreferences,
    UserPublic,
};
use serde::Deserialize;

use crate::auth::AuthProvider;
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

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<(HeaderMap, Json<AuthToken>), AppError> {
    let user = state
        .store
        .get_user_by_email(&req.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("邮箱或密码错误".into()))?;
    let hash = user.password_hash.as_ref().ok_or_else(|| AppError::Unauthorized("邮箱或密码错误".into()))?;
    if !state.auth.verify_password(&req.password, hash)? {
        return Err(AppError::Unauthorized("邮箱或密码错误".into()));
    }
    let groups = state.store.resolve_all_groups(&user.id).await.unwrap_or_else(|_| vec![user.role.clone()]);
    let access_token = state.auth.create_access_token(&user.id.0, &user.role, &groups, req.database.as_deref())?;
    let refresh_token = state.auth.create_refresh_token();
    let token_hash = state.auth.refresh_token_hash(&refresh_token);
    let expires_at = state.auth.refresh_expires_at();
    state.store.create_refresh_token(&uuid::Uuid::new_v4().to_string(), &user.id, &token_hash, &expires_at).await?;
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
