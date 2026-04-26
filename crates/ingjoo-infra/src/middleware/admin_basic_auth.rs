//! 数据库管理 API 的 HTTP Basic Auth 中间件
//!
//! 验证用户名固定为 "admin"，密码为 AppState 中的 admin_passwd。

use axum::extract::Request;
use axum::extract::State;
use axum::middleware::Next;
use axum::response::Response;

use crate::middleware::error::AppError;
use crate::AppState;

/// 验证数据库管理 API 的 HTTP Basic Auth
///
/// 用户名固定为 "admin"，密码为配置中的 admin_passwd。
/// 如果 multi-db 未启用或密码为空，拒绝所有请求。
pub async fn require_admin_basic_auth(
    State(state): State<std::sync::Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, AppError> {
    // 如果未启用多数据库模式，拒绝访问
    if !state.db_manager.is_multi_db_enabled() {
        return Err(AppError::Forbidden("多数据库模式未启用".into()));
    }

    // 密码为空时拒绝所有请求
    if state.admin_passwd.is_empty() {
        return Err(AppError::Forbidden("管理员密码未配置".into()));
    }

    // 从 Authorization header 提取 Basic Auth
    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("缺少 Authorization header".into()))?;

    // 验证 Basic Auth 格式
    let credentials = auth_header
        .strip_prefix("Basic ")
        .ok_or_else(|| AppError::Unauthorized("认证方式错误，需要 Basic Auth".into()))?;

    // base64 解码
    let decoded = base64_decode(credentials).ok_or_else(|| AppError::Unauthorized("Base64 解码失败".into()))?;

    let parts: Vec<&str> = decoded.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(AppError::Unauthorized("凭据格式错误".into()));
    }

    let username = parts[0];
    let password = parts[1];

    // 验证用户名和密码
    if username != "admin" || password != state.admin_passwd {
        return Err(AppError::Unauthorized("管理员密码错误".into()));
    }

    Ok(next.run(request).await)
}

/// 手动 base64 解码（避免引入额外依赖）
fn base64_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let decoded = base64_decode_manual(bytes)?;
    String::from_utf8(decoded).ok()
}

fn base64_decode_manual(input: &[u8]) -> Option<Vec<u8>> {
    const DECODE_TABLE: [i8; 128] = [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 62, -1, -1, -1, 63, 52, 53, 54, 55, 56, 57, 58, 59,
        60, 61, -1, -1, -1, -1, -1, -1, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
        21, 22, 23, 24, 25, -1, -1, -1, -1, -1, -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42,
        43, 44, 45, 46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1,
    ];

    let mut result = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;

    for &byte in input {
        if byte == b'=' {
            break;
        }
        let val = *DECODE_TABLE.get(byte as usize).unwrap_or(&-1);
        if val < 0 {
            return None;
        }
        buffer = (buffer << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            result.push((buffer >> bits) as u8);
        }
    }

    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_decode_valid() {
        // "admin:password" → base64 → "YWRtaW46cGFzc3dvcmQ="
        let result = base64_decode("YWRtaW46cGFzc3dvcmQ=");
        assert_eq!(result, Some("admin:password".to_string()));
    }

    #[test]
    fn test_base64_decode_invalid() {
        assert!(base64_decode("!!!invalid!!!").is_none());
    }

    #[test]
    fn test_base64_decode_empty() {
        let result = base64_decode("");
        assert_eq!(result, Some(String::new()));
    }
}
