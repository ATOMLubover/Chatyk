use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use axum::{body::Body, extract::State};
use jsonwebtoken::DecodingKey;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::util;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserToken {
    pub user_id: String,
    pub exp: i64,
}

/// `auth_middleware` checks for a valid Bear JWT token in the Authorization header.
#[instrument(skip(decoding_key, req, next))]
pub async fn auth_middleware(
    State(decoding_key): State<String>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let decoding_key = DecodingKey::from_secret(decoding_key.as_bytes());

    let user_token = util::decode_jwt::<UserToken>(token, &decoding_key).map_err(|err| {
        tracing::trace!("JWT decode error: {:?}", err);
        return StatusCode::UNAUTHORIZED;
    })?;

    req.extensions_mut().insert(user_token);

    return Ok(next.run(req).await);
}
