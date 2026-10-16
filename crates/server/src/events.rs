//! The events agency publishes as things happen (docs/api.md, 「Events」). Each kind of event is a type
//! of its own that names itself; a consumer subscribes to the kinds it reads and takes each from its own
//! stream at its own pace. Publishing never waits on a subscriber and reads nothing back, and an event
//! with no subscriber reaches nobody. A consumer across the boundary reads the same events by name, each as
//! its JSON, from a stream of the kinds it names (docs/api.md, 「Events」).

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

/// One kind of event, with the kebab-case name a consumer knows it by.
pub trait Event: Clone + Send + serde::Serialize + 'static {
  const NAME: &'static str;
}

type Senders = Vec<Box<dyn Any + Send>>;

/// One event as a consumer across the boundary reads it: its kind's name and its JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
  pub name: &'static str,
  pub body: serde_json::Value,
}

// One stream of named events, and the kinds it reads; none reads every kind.
struct Wire {
  kinds: Option<Vec<String>>,
  sender: UnboundedSender<Named>,
}

impl Wire {
  fn reads(&self, name: &str) -> bool {
    self.kinds.as_ref().is_none_or(|kinds| kinds.iter().any(|kind| kind == name))
  }
}

/// Every subscription, by the kind of event it reads. Cloned freely: every clone is the one bus.
#[derive(Clone, Default)]
pub struct Bus {
  subscribers: Arc<Mutex<HashMap<TypeId, Senders>>>,
  wires: Arc<Mutex<Vec<Wire>>>,
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

/// A stream of named events, of the kinds asked, from the moment it subscribed. Dropping it ends it.
pub struct WireSubscription {
  events: UnboundedReceiver<Named>,
}

impl WireSubscription {
  /// The next event, waiting for one; none once the bus is gone.
  pub async fn next(&mut self) -> Option<Named> {
    self.events.recv().await
  }

  /// The stream itself, for a reader that takes it as a stream.
  pub fn into_receiver(self) -> UnboundedReceiver<Named> {
    self.events
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

  /// A stream of the named events of the kinds given, every kind where none is given.
  pub fn subscribe_wire(&self, kinds: Option<Vec<String>>) -> WireSubscription {
    let (sender, events) = unbounded_channel();

    self
      .wires
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .push(Wire { kinds, sender });

    WireSubscription { events }
  }

  /// The event handed to every subscriber of its kind, and to nobody where there is none. A subscription
  /// dropped since is let go here.
  pub fn publish<E: Event>(&self, event: E) {
    self.publish_wire(&event);

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

  // The event handed, as its JSON, to every stream reading its kind; serialised only where one does.
  fn publish_wire<E: Event>(&self, event: &E) {
    let mut wires = self.wires.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    wires.retain(|wire| !wire.sender.is_closed());

    if !wires.iter().any(|wire| wire.reads(E::NAME)) {
      return;
    }

    let Ok(body) = serde_json::to_value(event) else { return };

    for wire in wires.iter().filter(|wire| wire.reads(E::NAME)) {
      let _ = wire.sender.send(Named {
        name: E::NAME,
        body: body.clone(),
      });
    }
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
