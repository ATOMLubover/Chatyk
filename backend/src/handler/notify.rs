use std::time::Duration;

use axum::{
    Extension,
    extract::State,
    response::{
        Sse,
        sse::{Event, KeepAlive},
    },
};
use futures::Stream;
use tokio::sync::broadcast;
use tokio_stream::{StreamExt, wrappers::BroadcastStream};

use crate::{
    handler::{AppError, AppState, middleware::UserToken},
    service::PushEvent,
};

pub async fn pull_notifications(
    State(state): State<AppState>,
    Extension(user_token): Extension<UserToken>,
) -> Sse<impl Stream<Item = Result<Event, AppError>>> {
    let user_id = user_token.user_id.clone();
    let user_id_clone = user_id.clone();

    tracing::trace!(
        "Establishing SSE stream connection for user_id: {}",
        user_id
    );

    // register the user to online users map if not exists
    let user_rx = state
        .online_users
        .entry(user_id)
        // TODO: configure the channel size
        .or_insert_with(|| {
            tracing::trace!(
                "Creating a new broadcast channel for user_id: {}",
                user_token.user_id
            );
            return broadcast::channel::<PushEvent>(128).0;
        })
        .subscribe();

    let inner_stream = BroadcastStream::new(user_rx).map(move |res| {
        tracing::trace!(
            "SSE event for user_id: {}, event: {:?}",
            user_id_clone,
            res.as_ref()
        );

        let msg = match res {
            Ok(msg) => msg,
            Err(err) => {
                tracing::error!("SSE broadcast stream error: {}", err);
                return Err(AppError::from(err));
            }
        };

        let msg = match serde_json::to_string(&msg) {
            Ok(msg) => msg,
            Err(err) => {
                tracing::error!("Failed to serialize PushEvent to JSON: {}", err);
                return Err(AppError::SerdeJsonError(err));
            }
        };

        return Ok(Event::default().data(msg));
    });

    let cleanup_stream = futures::stream::once(async move {
        // when the stream is done, remove the user from online users map
        tracing::trace!(
            "Closing SSE stream connection for user_id: {}",
            user_token.user_id
        );
        state.online_users.remove(&user_token.user_id);

        return Ok(Event::default().data("[=CLEANUP=]"));
    });

    return Sse::new(inner_stream.chain(cleanup_stream)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(5))
            .text("PING"),
    );
}
