//! The decisions agency hands up (docs/api.md, 「Hooks」). Each point a decision is asked at is a type of
//! its own, which names what it asks, what it is answered, and its default; a consumer registers its
//! answer at the point, and agency asks it there. A point nobody registered at answers its default,
//! never a failure.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

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
