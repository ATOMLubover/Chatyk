use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::ws::Message;
use axum::extract::{Request, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Extension, routing};
use axum::{Router, extract::State};
use axum_extra::TypedHeader;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use dashmap::DashMap;
use futures_util::SinkExt;
use futures_util::stream::StreamExt;
use jsonwebtoken::DecodingKey;
use serde::Deserialize;
use shared::util::{self};
use tokio::sync::broadcast::{self, Sender};

use crate::event::ServerEvent;
use crate::service::{self};
use crate::{AppConfig, CacheClient, DatabasePool};

pub type OnlineUsersMap = Arc<DashMap<String, Sender<ServerEvent>>>;

#[derive(Clone)]
pub struct AppState {
    online_users: OnlineUsersMap,
    database_pool: DatabasePool,
    cache_client: CacheClient,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TokenSubject {
    pub user_id: String,
}

pub fn get_router(
    config: &AppConfig,
    database: &DatabasePool,
    cache: &CacheClient,
) -> anyhow::Result<Router> {
    let jwt_decoding_key = std::env::var(&config.jwt_deckey_env)
        .map_err(|err| anyhow::anyhow!("JWT decoding key env not set: {}", err))?;

    let jwt_decoding_key = DecodingKey::from_secret(jwt_decoding_key.as_bytes());

    tracing::debug!("JWT decoding keys loaded from env");

    let event_router: Router<()> = Router::new()
        .route("/events", routing::get(exchange_events))
        .layer(axum::middleware::from_fn_with_state(
            jwt_decoding_key.clone(),
            auth_middleware,
        ))
        .with_state(AppState {
            online_users: Arc::new(DashMap::new()),
            database_pool: Arc::clone(database),
            cache_client: Arc::clone(cache),
        });

    let router = Router::new().nest("/api", event_router);

    return Ok(router);
}

pub async fn auth_middleware(
    State(decoding_key): State<DecodingKey>,
    TypedHeader(auth_header): TypedHeader<Authorization<Bearer>>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = auth_header.token().to_owned();

    let token = token
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token_sub: TokenSubject = util::decode_jwt(token, &decoding_key).map_err(|err| {
        tracing::trace!("JWT decode error: {:?}", err);
        return StatusCode::UNAUTHORIZED;
    })?;

    req.extensions_mut().insert(token_sub.user_id);

    return Ok(next.run(req).await);
}

pub async fn exchange_events(
    websock: WebSocketUpgrade,
    State(state): State<AppState>,
    Extension(user_id): Extension<String>,
) -> impl IntoResponse {
    tracing::trace!("WebSocket connection request for user_id: {}", user_id);

    let state_clone = state.clone();

    return websock.on_upgrade(move |mut socket| async move {
        tracing::debug!("WebSocket connection established for user_id: {}", &user_id);

        // send a initial ping to client
        if socket
            .send(Message::Ping(Bytes::from_static(&[1, 2, 3])))
            .await
            .is_err()
        {
            tracing::debug!("Failed to send initial ping to client, disconnecting");
            return;
        }

        match socket.recv().await {
            Some(Err(err)) => {
                tracing::debug!("WebSocket error: {:?}", err);
                return;
            }
            None => {
                tracing::debug!("WebSocket closed by user_id: {}", &user_id);
                return;
            }
            Some(Ok(msg)) => {
                tracing::debug!("Received initial message from client: {:?}", msg);
            }
        };

        // process websocket messages after ping-pong handshake
        let (mut sender, mut receiver) = socket.split();

        let mut rx = state_clone
            .online_users
            .entry(user_id.clone())
            .or_insert_with(|| broadcast::channel(64).0)
            .subscribe();

        let user_id_clone = user_id.clone();

        let mut send_task = tokio::task::spawn(async move {
            while let Ok(event) = rx.recv().await {
                if let Err(err) = service::handle_notified_event(event, &mut sender).await {
                    tracing::debug!(
                        "Failed to process event for user_id {}: {:?}",
                        &user_id_clone,
                        err
                    );

                    break;
                }
            }

            // when rx is closed, or some error occurs, close the websocket
            if let Err(err) = sender.send(Message::Close(None)).await {
                tracing::debug!("Failed to send close message to client: {:?}", err);
            }

            tracing::debug!("WebSocket send loop ended for user_id: {}", &user_id_clone);
        });

        let user_id_clone = user_id.clone();
        let online_users_clone = state_clone.online_users.clone();
        let pool_clone = state_clone.database_pool.clone();
        let cache_clone = state_clone.cache_client.clone();

        let mut recv_task = tokio::task::spawn(async move {
            while let Some(Ok(message)) = receiver.next().await {
                if let Err(err) = service::handle_recv_message(
                    user_id_clone.clone(),
                    message,
                    online_users_clone.clone(),
                    pool_clone.clone(),
                    cache_clone.clone(),
                )
                .await
                {
                    tracing::debug!(
                        "Error occurred when processing received message for user_id {}: {:?}",
                        &user_id_clone,
                        err
                    );

                    break;
                }
            }

            tracing::debug!(
                "WebSocket receive loop ended for user_id: {}",
                &user_id_clone
            );
        });

        // if any one of the tasks exit, abort the other
        tokio::select! {
            rv_a = (&mut send_task) => {
                if let Err(err) = rv_a {
                    tracing::debug!("Send task error for user_id {}: {:?}", &user_id, err);
                }
                recv_task.abort();
            },
            rv_b = (&mut recv_task) => {
                if let Err(err) = rv_b {
                    tracing::debug!("Receive task error for user_id {}: {:?}", &user_id, err);
                }
                send_task.abort();
            }
        }

        state_clone.online_users.remove(&user_id);

        tracing::debug!("WebSocket connection closed for user_id: {}", &user_id);
    });
}
