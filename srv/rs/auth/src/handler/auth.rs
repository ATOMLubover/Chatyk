use std::sync::Arc;

use axum::extract::State;
use axum::response::IntoResponse;
use axum::{Json, Router};
use diesel::RunQueryDsl;
use diesel::result::{DatabaseErrorKind, Error};
use shared::model::NewUser;
use shared::util::{self, JwtCodec};

use crate::dto::{ReqRegisterUser, ReqUserLogin, RspToken};
use crate::handler::{AppError, AppResult, AppState, TokenSubject};

pub fn get_router() -> Router<AppState> {
    use axum::Router;
    use axum::routing;

    return Router::new()
        .route("/register", routing::post(register_user))
        .route("/login", routing::post(login_user));
}

fn generate_token(subject: TokenSubject, codec: &JwtCodec) -> AppResult<String> {
    let token = codec.encode(subject)?;
    return Ok(token);
}

pub async fn register_user(
    State(state): State<AppState>,
    Json(payload): Json<ReqRegisterUser>,
) -> AppResult<impl IntoResponse> {
    use shared::schema::user_tbl::dsl::*;

    tracing::trace!("Register payload: {:?}", payload);

    let database_pool = Arc::clone(&state.database_pool);

    let token = tokio::task::spawn_blocking(move || {
        let conn = &mut database_pool.get()?;

        let hashed_password = bcrypt::hash(payload.password, bcrypt::DEFAULT_COST)?;
        let user_id = util::generate_id();

        // we need to ensure that token is generated before inserting user into database
        let token = generate_token(
            TokenSubject {
                user_id: user_id.clone(),
            },
            &state.inner.jwt_codec,
        )?;

        let new_user = NewUser {
            id: user_id.clone(),
            username: payload.username,
            password_hash: hashed_password,
        };

        match diesel::insert_into(user_tbl)
            .values(&new_user)
            .execute(conn)
        {
            Err(err) => {
                if let Error::DatabaseError(DatabaseErrorKind::UniqueViolation, info) = err {
                    tracing::debug!("Unique violation when register user: {:?}", info.message());
                    return Err(AppError::UsernameConflict);
                }
                return Err(AppError::DatabaseError(err));
            }
            Ok(_) => return Ok(token),
        };
    })
    .await??;

    return Ok(Json(RspToken {
        token_type: "Bearer".to_string(),
        token,
    }));
}

pub async fn login_user(
    State(state): State<AppState>,
    Json(payload): Json<ReqUserLogin>,
) -> AppResult<impl IntoResponse> {
    tracing::trace!("Login payload: {:?}", payload);

    let database_pool = Arc::clone(&state.database_pool);

    let token = tokio::task::spawn_blocking(move || {
        use diesel::prelude::*;
        use shared::schema::user_tbl::dsl::*;

        let conn = &mut database_pool.get()?;

        let (user_id, hashed_password): (String, String) = user_tbl
            .filter(username.eq(&payload.username))
            .select((id, password_hash))
            .first(conn)?;

        if !bcrypt::verify(&payload.password, &hashed_password)? {
            return Err(AppError::UsernameOrPasswordIncorrect);
        }

        let token = generate_token(
            TokenSubject {
                user_id: user_id.clone(),
            },
            &state.inner.jwt_codec,
        )?;

        return Ok(token);
    })
    .await??;

    return Ok(Json(RspToken {
        token_type: "Bearer".to_string(),
        token,
    }));
}
