use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use futures_util::StreamExt;

use crate::extractors::CurrentUser;
use crate::AppState;

pub async fn sse_events(
    State(state): State<Arc<AppState>>,
    user: CurrentUser,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events.subscribe();
    let uid = user.user_id.clone();
    let rx = Arc::new(tokio::sync::Mutex::new(rx));

    let stream = futures_util::stream::once(async { Ok(Event::default().data("{\"type\":\"connected\"}")) }).chain(
        futures_util::stream::unfold(rx, move |rx| {
            let uid = uid.clone();
            async move {
                let mut guard = rx.lock().await;
                loop {
                    let result = match guard.recv().await {
                        Ok(msg) => {
                            if msg.contains(&uid) || msg.contains("\"broadcast\"") {
                                Some(Ok(Event::default().data(msg)))
                            } else {
                                continue;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            Some(Ok(Event::default().data(format!("{{\"type\":\"lagged\",\"missed\":{}}}", n))))
                        }
                        Err(_) => None,
                    };
                    drop(guard);
                    return result.map(|ev| (ev, rx));
                }
            }
        }),
    );

    Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(30)).text("ping"))
}
