#![allow(
  dead_code,
  unused_imports,
  unused_variables,
  clippy::needless_lifetimes,
  clippy::let_and_return,
  clippy::unnecessary_to_owned
)]
//! The events agency publishes, read across the boundary (docs/api.md, 「Events」): one stream of server-sent
//! events, each named by its kind and carrying its JSON, of the kinds the caller names or every kind where
//! it names none, from the moment it is opened until the caller lets it go; and the events a verb's own
//! process published, posted to the server before the verb ends and published on its streams.

use std::convert::Infallible;

use std::borrow::Cow;

use axum::http::{Method, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use super::{Call, Refused, Route, answer};
use crate::events::Named;

/// The agency's one stream: server-sent events, or a page of them for a caller that polls.
pub struct EventStream;

impl Route for EventStream {
  const METHOD: Method = Method::GET;
  const PATH: &'static str = "/events";

  async fn answer(&self, call: Call) -> Result<Response, Refused> {
    let kinds: Option<Vec<String>> = call.query("kinds").map(|kinds| {
      kinds
        .split(',')
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .map(str::to_owned)
        .collect()
    });
    let after = call.query("after").and_then(|after| after.parse::<i64>().ok()).or_else(|| {
      call
        .request
        .headers()
        .get("last-event-id")
        .and_then(|id| id.to_str().ok())
        .and_then(|id| id.parse::<i64>().ok())
    });
    let polling = call
      .request
      .headers()
      .get(header::ACCEPT)
      .and_then(|accept| accept.to_str().ok())
      .is_some_and(|accept| accept.contains("application/json"));

    if polling {
      let limit = call
        .query("limit")
        .and_then(|limit| limit.parse::<usize>().ok())
        .unwrap_or(200)
        .min(1000);
      let (events, more) = call.events().page(after, kinds.as_deref(), limit);
      let items: Vec<Value> = events.iter().map(Named::envelope).collect();

      return Ok(answer(StatusCode::OK, json!({ "items": items, "more": more })));
    }

    let subscription = call.events().subscribe_wire(kinds);
    let stream =
      futures_util::stream::unfold((subscription, after), |(mut subscription, after)| async move {
        loop {
          let named = subscription.next().await?;
          if after.is_some_and(|after| named.id <= after) {
            continue;
          }
          let event = Event::default()
            .id(named.id.to_string())
            .event(named.name.to_string())
            .json_data(named.envelope())
            .unwrap_or_default();

          return Some((Ok::<_, Infallible>(event), (subscription, after)));
        }
      });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()).into_response())
  }
}

crate::moved!(EventStream);

/// Whether a word is an event's kind as agency names one: lowercase letters, digits and hyphens, between
/// dots, as `session.created` and `job.ended`.
pub fn is_kind(word: &str) -> bool {
  !word.is_empty()
    && word.split('.').all(|part| {
      !part.is_empty()
        && part.split('-').all(|part| {
          !part.is_empty()
            && part.chars().all(|letter| letter.is_ascii_lowercase() || letter.is_ascii_digit())
        })
    })
}

/// An event a process of the agency's own published, published on the server's streams as it stands.
pub struct EventPublish;

impl Route for EventPublish {
  const METHOD: Method = Method::POST;
  const PATH: &'static str = "/events";

  async fn answer(&self, mut call: Call) -> Result<Response, Refused> {
    let posted = call.body().await?;
    let Some(kind) = posted.get("kind").and_then(Value::as_str).filter(|kind| is_kind(kind)) else {
      return Err(Refused::new(StatusCode::BAD_REQUEST, "kind (an event's name) is required"));
    };
    let data = posted.get("data").cloned().unwrap_or(Value::Null);

    call.events().publish_named(Named {
      id: 0,
      at: 0,
      name: Cow::Owned(kind.to_owned()),
      body: data.clone(),
    });

    Ok(answer(StatusCode::CREATED, json!({ "kind": kind, "data": data })))
  }
}

crate::moved!(EventPublish);
