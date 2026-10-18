//! What a session's job turn has written of its answer so far (docs/api.md, 「Sessions」, the output read):
//! every text block the program's answers wrote in the turn's record, in order, joined as the turn's
//! words, kept from the turn's start past its end until the session's next turn begins or the session
//! ends, so a turn stopped from outside still leaves what it wrote. An attempt that goes out again on
//! another login or a fresh session begins its words again.

use std::collections::HashMap;
use std::sync::Mutex;

use agent_session::{Output, SessionId};

// One session's latest turn: its run, whether it still goes, and the text blocks it wrote.
struct Written {
  run: String,
  running: bool,
  texts: Vec<String>,
}

/// What every session's latest job turn wrote.
#[derive(Default)]
pub struct Outputs {
  written: Mutex<HashMap<SessionId, Written>>,
}

impl Outputs {
  fn written(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, Written>> {
    self.written.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  /// A session's turn begun under its run, in place of whatever its last one wrote.
  pub fn begun(&self, session: &SessionId, run: &str) {
    self.written().insert(
      session.clone(),
      Written {
        run: run.to_owned(),
        running: true,
        texts: Vec::new(),
      },
    );
  }

  /// An attempt of the turn going out, its words begun again.
  pub fn attempted(&self, session: &SessionId) {
    if let Some(written) = self.written().get_mut(session) {
      written.texts.clear();
    }
  }

  /// The text blocks the turn's program wrote since it was last read.
  pub fn wrote(&self, session: &SessionId, texts: &[String]) {
    if let Some(written) = self.written().get_mut(session) {
      written.texts.extend_from_slice(texts);
    }
  }

  /// The turn ended, what it wrote kept.
  pub fn ended(&self, session: &SessionId) {
    if let Some(written) = self.written().get_mut(session) {
      written.running = false;
    }
  }

  /// The session gone, and what its turns wrote with it.
  pub fn forget(&self, session: &SessionId) {
    self.written().remove(session);
  }

  /// What the session's turn now going, or its last one, has written; none where it carried no job turn.
  pub fn read(&self, session: &SessionId) -> Option<Output> {
    self.written().get(session).map(|written| Output {
      run: written.run.clone(),
      running: written.running,
      text: written.texts.join("\n\n"),
    })
  }
}
