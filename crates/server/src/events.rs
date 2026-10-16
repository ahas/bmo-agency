//! The events agency publishes as things happen (docs/api.md, 「Events」). Each kind of event is a type
//! of its own that names itself; a consumer subscribes to the kinds it reads and takes each from its own
//! stream at its own pace. Publishing never waits on a subscriber and reads nothing back, and an event
//! with no subscriber reaches nobody.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// One kind of event, with the kebab-case name a consumer knows it by.
pub trait Event: Clone + Send + 'static {
  const NAME: &'static str;
}

type Senders = Vec<Box<dyn Any + Send>>;

/// Every subscription, by the kind of event it reads. Cloned freely: every clone is the one bus.
#[derive(Clone, Default)]
pub struct Bus {
  subscribers: Arc<Mutex<HashMap<TypeId, Senders>>>,
}

/// One consumer's stream of one kind of event, from the moment it subscribed. Dropping it ends the
/// subscription.
pub struct Subscription<E> {
  events: UnboundedReceiver<E>,
}

impl<E: Event> Subscription<E> {
  /// The next event, waiting for one; none once the bus is gone.
  pub async fn next(&mut self) -> Option<E> {
    self.events.recv().await
  }

  /// The next event where one is waiting, without waiting for one.
  pub fn ready(&mut self) -> Option<E> {
    self.events.try_recv().ok()
  }
}

impl Bus {
  fn held(&self) -> MutexGuard<'_, HashMap<TypeId, Senders>> {
    self.subscribers.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  /// A subscription to every event of one kind published from now on.
  pub fn subscribe<E: Event>(&self) -> Subscription<E> {
    let (sender, events) = unbounded_channel::<E>();

    self.held().entry(TypeId::of::<E>()).or_default().push(Box::new(sender));

    Subscription { events }
  }

  /// The event handed to every subscriber of its kind, and to nobody where there is none. A subscription
  /// dropped since is let go here.
  pub fn publish<E: Event>(&self, event: E) {
    let mut subscribers = self.held();
    let Some(senders) = subscribers.get_mut(&TypeId::of::<E>()) else {
      return;
    };

    senders.retain(|sender| {
      sender
        .downcast_ref::<UnboundedSender<E>>()
        .is_some_and(|sender| sender.send(event.clone()).is_ok())
    });
  }

  /// How many subscriptions to one kind of event stand.
  pub fn subscribers<E: Event>(&self) -> usize {
    let mut subscribers = self.held();
    let Some(senders) = subscribers.get_mut(&TypeId::of::<E>()) else {
      return 0;
    };

    senders.retain(|sender| {
      sender.downcast_ref::<UnboundedSender<E>>().is_some_and(|sender| !sender.is_closed())
    });
    senders.len()
  }
}
