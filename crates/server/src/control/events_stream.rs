//! The events agency publishes, read across the boundary (docs/api.md, 「Events」): one stream of server-sent
//! events, each named by its kind and carrying its JSON, of the kinds the caller names or every kind where
//! it names none, from the moment it is opened until the caller lets it go; and the events a verb's own
//! process published, posted to the server before the verb ends and published on its streams.

use std::convert::Infallible;

use std::borrow::Cow;

use axum::http::{Method, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::events::Named;

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

/// Whether a word is an event's kind as agency names one: kebab-case, lowercase letters and digits between
/// single hyphens.
pub fn is_kind(word: &str) -> bool {
  !word.is_empty()
    && word.split('-').all(|part| {
      !part.is_empty() && part.chars().all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit())
    })
}

/// An event a verb's own process published, published on the server's streams as it stands.
pub struct EventPublish;

impl Route for EventPublish {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/events/publish";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let posted = call.body().await?;
    let Some(kind) = posted.get("kind").and_then(Value::as_str).filter(|kind| is_kind(kind)) else {
      return Err(Refused::new(StatusCode::BAD_REQUEST, "kind (an event's kebab-case name) is required"));
    };
    let body = posted.get("body").cloned().unwrap_or(Value::Null);

    call.events().publish_named(Named {
      name: Cow::Owned(kind.to_owned()),
      body,
    });

    Ok(answer(StatusCode::OK, json!({ "ok": true })))
  }
}

crate::moved!(EventPublish);
