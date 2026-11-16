//! The far end's side of the road home on a person's PC (docs/person-nodes.md, 「The link over jake」): the ports
//! the near end names, listened on at the PC's loopback beside the programs, and the node's bridge socket; each
//! connection taken there held while the near end is asked over the mirror to dial it home, and joined to the
//! stream the near end opens for it.

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::frame::{Frame, Home, Refused};

/// How long a connection taken waits to be dialed home before it is closed.
pub const DIAL_WAIT: Duration = Duration::from_secs(20);

/// How often a port the far end could not listen on is tried again.
pub const LISTEN_AGAIN: Duration = Duration::from_secs(30);

/// A connection either end joins: a TCP connection or one to a unix socket.
pub trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}

impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

enum Listening {
  Standing(JoinHandle<()>),
  Refused(String),
}

impl Drop for Listening {
  fn drop(&mut self) {
    if let Self::Standing(accepting) = self {
      accepting.abort();
    }
  }
}

/// What the far end carries home: the ports it was asked for and how each stands, the mirror session that dials,
/// and the connections waiting to be dialed, by the number each was given.
pub struct Carrier {
  host: String,
  session: Mutex<Option<mpsc::UnboundedSender<Frame>>>,
  listens: Mutex<BTreeMap<u16, Listening>>,
  told: Mutex<Option<Vec<Refused>>>,
  held: Mutex<HashMap<u64, Box<dyn Stream>>>,
  next: AtomicU64,
}

impl Carrier {
  /// A carrier listening at the loopback address `host`, the one the mirror is spoken on.
  pub fn new(host: &str) -> Arc<Self> {
    Arc::new(Self {
      host: host.to_owned(),
      session: Mutex::new(None),
      listens: Mutex::new(BTreeMap::new()),
      told: Mutex::new(None),
      held: Mutex::new(HashMap::new()),
      next: AtomicU64::new(1),
    })
  }

  /// The mirror session that dials from now on.
  pub fn attach(&self, session: mpsc::UnboundedSender<Frame>) {
    if let Ok(mut current) = self.session.lock() {
      *current = Some(session);
    }
  }

  /// The mirror session ended: it dials no more, unless another has taken its place.
  pub fn detach(&self, session: &mpsc::UnboundedSender<Frame>) {
    if let Ok(mut current) = self.session.lock()
      && current.as_ref().is_some_and(|standing| standing.same_channel(session))
    {
      *current = None;
    }
  }

  fn say(&self, frame: Frame) {
    if let Ok(current) = self.session.lock()
      && let Some(session) = current.as_ref()
    {
      let _ = session.send(frame);
    }
  }

  /// The ports asked for listened on, one already standing kept and one no longer asked let go, and which could
  /// not be listened on told to the session: always where `tell` says, else only where it changed.
  pub async fn carry(self: &Arc<Self>, ports: Option<&[u16]>, tell: bool) {
    let wanted: Vec<u16> = {
      let Ok(mut listens) = self.listens.lock() else { return };

      if let Some(ports) = ports {
        listens.retain(|port, _| ports.contains(port));

        for port in ports {
          listens.entry(*port).or_insert_with(|| Listening::Refused(String::new()));
        }
      }

      listens
        .iter()
        .filter(|(_, listening)| matches!(listening, Listening::Refused(_)))
        .map(|(port, _)| *port)
        .collect()
    };

    for port in wanted {
      let listening = match tokio::net::TcpListener::bind((self.host.as_str(), port)).await {
        Ok(listener) => Listening::Standing(tokio::spawn(take_port(listener, self.clone(), port))),
        Err(why) => Listening::Refused(why.to_string()),
      };

      if let Ok(mut listens) = self.listens.lock()
        && let Some(slot) = listens.get_mut(&port)
      {
        *slot = listening;
      }
    }

    let refused: Vec<Refused> = match self.listens.lock() {
      Ok(listens) => listens
        .iter()
        .filter_map(|(port, listening)| match listening {
          Listening::Refused(why) => Some(Refused {
            port: *port,
            why: why.clone(),
          }),
          Listening::Standing(_) => None,
        })
        .collect(),
      Err(_) => return,
    };
    let changed = match self.told.lock() {
      Ok(mut told) => {
        let changed = told.as_ref() != Some(&refused);

        *told = Some(refused.clone());
        changed
      }
      Err(_) => true,
    };

    if tell || changed {
      self.say(Frame::Carried { refused });
    }
  }

  /// A connection taken, held under a number of its own while the session is asked to dial it home, and closed
  /// where no session stands or nothing dials it in time.
  pub fn hold(self: &Arc<Self>, connection: Box<dyn Stream>, to: Home) {
    let id = self.next.fetch_add(1, Ordering::Relaxed);
    let dialing = self.session.lock().ok().and_then(|current| current.clone());
    let Some(dialing) = dialing else { return };

    if let Ok(mut held) = self.held.lock() {
      held.insert(id, connection);
    }

    if dialing.send(Frame::Dial { id, to }).is_err() {
      self.take(id);
      return;
    }

    let carrier = self.clone();

    tokio::spawn(async move {
      tokio::time::sleep(DIAL_WAIT).await;
      carrier.take(id);
    });
  }

  /// The connection held under a number, taken from the held.
  pub fn take(&self, id: u64) -> Option<Box<dyn Stream>> {
    self.held.lock().ok().and_then(|mut held| held.remove(&id))
  }
}

// Each connection taken on a carried port held to be dialed home.
async fn take_port(listener: tokio::net::TcpListener, carrier: Arc<Carrier>, port: u16) {
  while let Ok((connection, _)) = listener.accept().await {
    carrier.hold(Box::new(connection), Home::Port(port));
  }
}

/// Each connection taken on the node's bridge socket held to be dialed home.
pub async fn take_bridge(listener: tokio::net::UnixListener, carrier: Arc<Carrier>) {
  while let Ok((connection, _)) = listener.accept().await {
    carrier.hold(Box::new(connection), Home::Bridge);
  }
}

/// The ports that could not be listened on tried again every while, the session told where that changed.
pub async fn listen_again(carrier: Arc<Carrier>) {
  loop {
    tokio::time::sleep(LISTEN_AGAIN).await;
    carrier.carry(None, false).await;
  }
}
