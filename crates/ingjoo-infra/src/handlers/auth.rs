use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Extension;
use axum::Json;
use ingjoo_core::{AuthToken, LoginRequest, RegisterRequest, UpdateProfileRequest, User, UserId, UserPublic};
use serde::Deserialize;

use crate::auth::AuthProvider;
use crate::middleware::error::AppError;
use ingjoo_core::db::error::StoreError;
use crate::AppState;

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<AuthToken>), AppError> {
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
    let user = state.store.create_user(&user).await
        .map_err(|e| match e {
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
    state
        .store
        .create_refresh_token(
            &uuid::Uuid::new_v4().to_string(),
            &user.id,
            &token_hash,
            &expires_at,
        )
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(AuthToken {
            access_token,
            refresh_token,
            user: UserPublic::from(&user),
        }),
    ))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthToken>, AppError> {
    let user = state
        .store
        .get_user_by_email(&req.email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("邮箱或密码错误".into()))?;
    let hash = user
        .password_hash
        .as_ref()
        .ok_or_else(|| AppError::Unauthorized("邮箱或密码错误".into()))?;
    if !state.auth.verify_password(&req.password, hash)? {
        return Err(AppError::Unauthorized("邮箱或密码错误".into()));
    }
    let groups = state.store.resolve_all_groups(&user.id).await.unwrap_or_else(|_| vec![user.role.clone()]);
    let access_token = state.auth.create_access_token(&user.id.0, &user.role, &groups, req.database.as_deref())?;
    let refresh_token = state.auth.create_refresh_token();
    let token_hash = state.auth.refresh_token_hash(&refresh_token);
    let expires_at = state.auth.refresh_expires_at();
    state
        .store
        .create_refresh_token(
            &uuid::Uuid::new_v4().to_string(),
            &user.id,
            &token_hash,
            &expires_at,
        )
        .await?;
    Ok(Json(AuthToken {
        access_token,
        refresh_token,
        user: UserPublic::from(&user),
    }))
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<AuthToken>, AppError> {
    let token_hash = state.auth.refresh_token_hash(&body.refresh_token);
    let (user_id_str, _expires_at) = state
        .store
        .get_refresh_token(&token_hash)
        .await?
        .ok_or_else(|| AppError::Unauthorized("无效的刷新令牌".into()))?;
    let user_id = UserId::new(user_id_str);
    let user = state
        .store
        .get_user_by_id(&user_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("用户不存在".into()))?;
    state.store.delete_refresh_token(&token_hash).await?;
    let groups = state.store.resolve_all_groups(&user.id).await.unwrap_or_else(|_| vec![user.role.clone()]);
    let access_token = state.auth.create_access_token(&user.id.0, &user.role, &groups, None)?;
    let new_refresh = state.auth.create_refresh_token();
    let new_hash = state.auth.refresh_token_hash(&new_refresh);
    let expires_at = state.auth.refresh_expires_at();
    state
        .store
        .create_refresh_token(
            &uuid::Uuid::new_v4().to_string(),
            &user.id,
            &new_hash,
            &expires_at,
        )
        .await?;
    Ok(Json(AuthToken {
        access_token,
        refresh_token: new_refresh,
        user: UserPublic::from(&user),
    }))
}

pub async fn get_profile(
    Extension(current_user): Extension<crate::extractors::CurrentUser>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<UserPublic>, AppError> {
    let user_id = UserId::new(current_user.user_id);
    let user = state
        .store
        .get_user_by_id(&user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
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
        .update_user(
            &user_id,
            req.name.as_deref(),
            req.avatar_url.as_deref(),
            req.bio.as_deref(),
        )
        .await?
        .ok_or_else(|| AppError::NotFound("用户不存在".into()))?;
    Ok(Json(UserPublic::from(&user)))
}
