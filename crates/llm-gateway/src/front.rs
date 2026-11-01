//! The gateway's front: the one address every run and every gate asks, which streams each request
//! through to the LiteLLM child it serves from and each answer back as they stand, reading neither. A
//! new table is a new child: once it answers, new requests go to it, and the old child is stopped only
//! when the requests in hand through it have ended, however long they take, so a saved change never
//! cuts a request.

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use http_body_util::BodyDataStream;
use hyper_util::rt::TokioIo;
use serde::Serialize;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One LiteLLM process serving one table, ready to be asked.
pub trait Child: Send + Sync {
  fn port(&self) -> u16;
  fn stop(&self) -> BoxFuture<'_, ()>;
}

/// What the console is told of the gateway: which table it serves, whether a new one is starting, and
/// how many old children are still finishing their requests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Status {
  pub serving: Option<String>,
  pub starting: Option<String>,
  pub draining: usize,
}

struct Serving {
  table: String,
  child: Box<dyn Child>,
  in_hand: AtomicUsize,
  retired: AtomicBool,
}

#[derive(Default)]
struct Held {
  current: Option<Arc<Serving>>,
  retired: Vec<Arc<Serving>>,
  starting: Option<String>,
}

pub struct Front {
  held: Mutex<Held>,
  on_status: Box<dyn Fn(&Status) + Send + Sync>,
}

// The hop-by-hop headers, which belong to one connection and are not handed on, and the ones a hop
// settles for itself.
const UNPASSED: [&str; 6] =
  ["host", "connection", "keep-alive", "transfer-encoding", "upgrade", "content-length"];

// A task taken down when its holder goes: the connection to a child, closed with the request it carries.
struct Owned(JoinHandle<()>);

impl Drop for Owned {
  fn drop(&mut self) {
    self.0.abort();
  }
}

// A request in hand through one child, let go when the answer ends however it ends.
struct InHand {
  front: Arc<Front>,
  serving: Arc<Serving>,
}

impl Drop for InHand {
  fn drop(&mut self) {
    self.serving.in_hand.fetch_sub(1, Ordering::SeqCst);
    let (front, serving) = (Arc::clone(&self.front), Arc::clone(&self.serving));
    // Outside a runtime — the process ending — nothing is left to stop it for.
    if let Ok(runtime) = tokio::runtime::Handle::try_current() {
      runtime.spawn(async move { front.stop_if_idle(&serving).await });
    }
  }
}

fn text(status: StatusCode, said: String) -> Response {
  (status, [(header::CONTENT_TYPE, "text/plain;charset=utf-8")], said).into_response()
}

impl Front {
  pub fn new(on_status: impl Fn(&Status) + Send + Sync + 'static) -> Arc<Front> {
    Arc::new(Front {
      held: Mutex::new(Held::default()),
      on_status: Box::new(on_status),
    })
  }

  fn status_of(held: &Held) -> Status {
    Status {
      serving: held.current.as_ref().map(|serving| serving.table.clone()),
      starting: held.starting.clone(),
      draining: held.retired.len(),
    }
  }

  pub fn status(&self) -> Status {
    Front::status_of(&self.held.lock().unwrap())
  }

  fn report(&self) {
    let status = self.status();
    (self.on_status)(&status);
  }

  /// The table the front is to serve. A table already served, or a start already under way, starts
  /// nothing; a new one starts its child, waits for it, and takes over from the old one, which drains.
  pub async fn take(
    &self,
    table: &str,
    start: impl Future<Output = Result<Box<dyn Child>, String>>,
  ) -> Result<(), String> {
    {
      let mut held = self.held.lock().unwrap();
      if held.current.as_ref().is_some_and(|serving| serving.table == table) || held.starting.is_some() {
        return Ok(());
      }
      held.starting = Some(table.to_owned());
    }
    self.report();

    let started = start.await;
    self.held.lock().unwrap().starting = None;
    self.report();
    let child = started?;

    let old = {
      let mut held = self.held.lock().unwrap();
      let old = held.current.replace(Arc::new(Serving {
        table: table.to_owned(),
        child,
        in_hand: AtomicUsize::new(0),
        retired: AtomicBool::new(false),
      }));
      if let Some(old) = &old {
        old.retired.store(true, Ordering::SeqCst);
        held.retired.push(Arc::clone(old));
      }
      old
    };
    if let Some(old) = old {
      self.stop_if_idle(&old).await;
    }
    self.report();
    Ok(())
  }

  /// One request, carried to the child serving now and its answer carried back as it comes.
  pub async fn handle(self: &Arc<Self>, request: Request) -> Response {
    // Counted in hand under the lock the child is taken under, so a take that retires that child in
    // between never finds it idle and stops it under the request.
    let serving = {
      let held = self.held.lock().unwrap();
      let Some(serving) = held.current.clone() else {
        return text(StatusCode::SERVICE_UNAVAILABLE, "the gateway has no table to serve yet".to_owned());
      };
      serving.in_hand.fetch_add(1, Ordering::SeqCst);
      serving
    };
    let in_hand = InHand {
      front: Arc::clone(self),
      serving: Arc::clone(&serving),
    };
    let port = serving.child.port();

    let (mut parts, body) = request.into_parts();
    for name in UNPASSED {
      parts.headers.remove(name);
    }
    // The answer goes back byte for byte, so it is asked for as it is written.
    parts.headers.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("identity"));
    parts
      .headers
      .insert(header::HOST, HeaderValue::from_str(&format!("127.0.0.1:{port}")).expect("an address"));
    let target = parts.uri.path_and_query().map_or("/", |at| at.as_str()).to_owned();
    parts.uri = target.parse().expect("a path and query read off a request");
    let upstream = hyper::Request::from_parts(parts, body);

    // No wait of the front's own: a slow card may write nothing for minutes, and the request's own wait
    // is the child's to keep.
    let asked = async {
      let stream = TcpStream::connect(("127.0.0.1", port)).await.map_err(|fault| fault.to_string())?;
      let (mut sender, connection) = hyper::client::conn::http1::handshake(TokioIo::new(stream))
        .await
        .map_err(|fault| fault.to_string())?;
      let connection = Owned(tokio::spawn(async move {
        let _ = connection.await;
      }));
      let answer = sender.send_request(upstream).await.map_err(|fault| fault.to_string())?;
      Ok::<_, String>((answer, connection))
    };
    let (answer, connection) = match asked.await {
      Ok(asked) => asked,
      Err(fault) => {
        drop(in_hand);
        return text(StatusCode::BAD_GATEWAY, format!("the gateway could not reach its table: {fault}"));
      }
    };

    let (mut parts, body) = answer.into_parts();
    for name in UNPASSED.iter().chain(&["content-encoding"]) {
      parts.headers.remove(*name);
    }
    // The request stays in hand, and its connection to the child open, until its answer has been
    // carried whole or its caller has gone.
    let kept = (in_hand, connection);
    let streamed = BodyDataStream::new(body).map(move |chunk| {
      let _ = &kept;
      chunk
    });
    Response::from_parts(parts, Body::from_stream(streamed))
  }

  /// Every child stopped, the one serving and the ones draining.
  pub async fn close(&self) {
    let all: Vec<Arc<Serving>> = {
      let mut held = self.held.lock().unwrap();
      let mut all: Vec<Arc<Serving>> = std::mem::take(&mut held.retired);
      all.extend(held.current.take());
      all
    };
    futures_util::future::join_all(all.iter().map(|serving| serving.child.stop())).await;
    self.report();
  }

  async fn stop_if_idle(&self, serving: &Arc<Serving>) {
    {
      let mut held = self.held.lock().unwrap();
      if !serving.retired.load(Ordering::SeqCst) || serving.in_hand.load(Ordering::SeqCst) > 0 {
        return;
      }
      let Some(at) = held.retired.iter().position(|one| Arc::ptr_eq(one, serving)) else {
        return;
      };
      held.retired.remove(at);
    }
    serving.child.stop().await;
    self.report();
  }
}

/// Every path and method comes through the front.
pub fn router(front: Arc<Front>) -> Router {
  Router::new()
    .fallback(
      |State(front): State<Arc<Front>>, request: Request| async move { front.handle(request).await },
    )
    .with_state(front)
}
