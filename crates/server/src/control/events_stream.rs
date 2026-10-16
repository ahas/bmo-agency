//! The events agency publishes, read across the boundary (docs/api.md, 「Events」): one stream of
//! server-sent events, each named by its kind and carrying its JSON, of the kinds the caller names or every
//! kind where it names none, from the moment it is opened until the caller lets it go.

use std::convert::Infallible;

use axum::http::Method;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};

use super::{Call, Refused, Route};

/// The stream of agency's events.
pub struct EventStream;

impl Route for EventStream {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/events";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let kinds = call.query("kinds").map(|kinds| {
      kinds
        .split(',')
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .map(str::to_owned)
        .collect()
    });
    let subscription = call.events().subscribe_wire(kinds);
    let stream = futures_util::stream::unfold(subscription, |mut subscription| async move {
      let named = subscription.next().await?;
      let event = Event::default().event(named.name).json_data(&named.body).unwrap_or_default();

      Some((Ok::<_, Infallible>(event), subscription))
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()).into_response())
  }
}

crate::moved!(EventStream);
