//! The decisions agency hands up (docs/api.md, 「Hooks」). Each point a decision is asked at is a type of
//! its own, which names what it asks, what it is answered, and its default; a consumer registers its
//! answer at the point, and agency asks it there. A point nobody registered at answers its default,
//! never a failure. A consumer across the boundary registers at a point by its name and an address: agency
//! posts each ask to it as JSON and reads its answer back, and a call that fails answers the default.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::json;

/// One point a decision is asked at.
pub trait HookPoint: 'static {
  /// What agency hands the consumer.
  type Ask: Send + 'static;
  /// What the consumer answers.
  type Answer: Send + 'static;
  /// The kebab-case name the point is known by.
  const NAME: &'static str;

  /// What the point answers where nobody registered at it: a default that does nothing.
  fn default_answer(ask: Self::Ask) -> Self::Answer;
}

/// A registered answer's future.
pub type Answering<A> = Pin<Box<dyn Future<Output = A> + Send>>;

type Registered<H> =
  Arc<dyn Fn(<H as HookPoint>::Ask) -> Answering<<H as HookPoint>::Answer> + Send + Sync>;

/// Every answer registered, by its point. Cloned freely: every clone is the one set of hooks.
#[derive(Clone, Default)]
pub struct Hooks {
  registered: Arc<RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>>,
}

impl Hooks {
  /// The consumer's answer at a point, standing in for any registered there before.
  pub fn register<H, Answer, Answered>(&self, answer: Answer)
  where
    H: HookPoint,
    Answer: Fn(H::Ask) -> Answered + Send + Sync + 'static,
    Answered: Future<Output = H::Answer> + Send + 'static,
  {
    let boxed: Registered<H> = Arc::new(move |ask| Box::pin(answer(ask)));

    self
      .registered
      .write()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .insert(TypeId::of::<H>(), Box::new(boxed));
  }

  /// The point's registered answer taken away, so it answers its default again.
  pub fn unregister<H: HookPoint>(&self) {
    self
      .registered
      .write()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .remove(&TypeId::of::<H>());
  }

  /// Whether a consumer registered at the point.
  pub fn registered<H: HookPoint>(&self) -> bool {
    self
      .registered
      .read()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .contains_key(&TypeId::of::<H>())
  }

  /// The decision asked at a point: the registered answer's, or the point's default.
  pub async fn ask<H: HookPoint>(&self, ask: H::Ask) -> H::Answer {
    let registered = self
      .registered
      .read()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .get(&TypeId::of::<H>())
      .and_then(|answer| answer.downcast_ref::<Registered<H>>())
      .cloned();

    match registered {
      Some(answer) => answer(ask).await,
      None => H::default_answer(ask),
    }
  }
}

/// A consumer's answer at a point, reached at its address: each ask posted as `{ point, ask }`, under its
/// token where it names one, and the answer read from the `answer` of what it answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
  pub url: String,
  pub token: Option<String>,
}

// How long an ask waits on a consumer before the point answers its default.
const REMOTE_WAIT: Duration = Duration::from_secs(10);

/// One point a consumer across the boundary may register at, by its name.
pub struct Wired {
  pub name: &'static str,
  pub register: fn(&Hooks, Remote),
  pub unregister: fn(&Hooks),
  pub registered: fn(&Hooks) -> bool,
}

inventory::collect!(Wired);

/// Makes a point one a consumer across the boundary registers at: `crate::wired!(RoomTitle);`
#[macro_export]
macro_rules! wired {
  ($point:ty) => {
    $crate::control::inventory::submit! {
      $crate::hooks::Wired {
        name: <$point as $crate::hooks::HookPoint>::NAME,
        register: $crate::hooks::register_remote::<$point>,
        unregister: $crate::hooks::Hooks::unregister::<$point>,
        registered: $crate::hooks::Hooks::registered::<$point>,
      }
    }
  };
}

/// Every point a consumer across the boundary may register at.
pub fn wired() -> impl Iterator<Item = &'static Wired> {
  inventory::iter::<Wired>.into_iter()
}

/// The point a consumer across the boundary names, where there is one.
pub fn wired_named(name: &str) -> Option<&'static Wired> {
  wired().find(|point| point.name == name)
}

/// A consumer's address registered at a point: every ask there posted to it.
pub fn register_remote<H>(hooks: &Hooks, remote: Remote)
where
  H: HookPoint,
  H::Ask: Serialize,
  H::Answer: DeserializeOwned,
{
  let client = reqwest::Client::new();
  let remote = Arc::new(remote);

  hooks.register::<H, _, _>(move |ask: H::Ask| {
    let (client, remote) = (client.clone(), remote.clone());

    // The ask is written out before anything is awaited, and kept for the default.
    let body = serde_json::to_value(&ask).map(|ask| json!({ "point": H::NAME, "ask": ask }));

    async move {
      let answered = match body {
        Ok(body) => asked::<H>(&client, &remote, &body).await,
        Err(why) => Err(why.to_string()),
      };

      match answered {
        Ok(answer) => answer,
        Err(why) => {
          eprintln!("hook {}: {} did not answer ({why}); the default stands", H::NAME, remote.url);
          H::default_answer(ask)
        }
      }
    }
  });
}

async fn asked<H>(
  client: &reqwest::Client,
  remote: &Remote,
  body: &serde_json::Value,
) -> Result<H::Answer, String>
where
  H: HookPoint,
  H::Answer: DeserializeOwned,
{
  let mut call = client.post(&remote.url).timeout(REMOTE_WAIT).json(body);

  if let Some(token) = &remote.token {
    call = call.bearer_auth(token);
  }

  let answer = call.send().await.map_err(|why| why.to_string())?;
  let status = answer.status();

  if !status.is_success() {
    return Err(format!("it answered {status}"));
  }

  let mut body: serde_json::Value = answer.json().await.map_err(|why| why.to_string())?;

  serde_json::from_value(body["answer"].take()).map_err(|why| why.to_string())
}
