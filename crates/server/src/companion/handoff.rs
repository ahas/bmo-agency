//! The session handed over at the handoff mark (docs/companions.md, 「Its limit」): one more turn of it, in no
//! round of the room's and on the very run its turns go out as, asking it for its handoff note; and where it
//! wrote one, a new generation holding the note for its first turn. An answer of none, an empty one or a
//! failed turn leaves the session as it was.

use std::time::Instant;

use indexmap::IndexMap;
use policy::companion_runner::COMPANION_RUNNER;
use policy::fill_marks;
use serde_json::Value;

use super::going::going;
use super::overtaken::{self, ResetHold};
use super::run::{NoAnswer, Reply, run_turn};
use super::sessions::{self, Kept};
use crate::door::record::UsageSpent;
use crate::lines::companion::{CompanionTurn, compose};
use crate::places::Ground;
use crate::run_log::lines::Line;
use crate::run_log::{RunLog, Target};
use crate::text::{length, trim};
use crate::worker_session::records::{note_answer, slot_account};

/// The companion a handoff is for: its agent name, the label it is shown under, and the model it is pinned
/// to.
pub struct Companion {
  pub name: String,
  pub label: Option<String>,
  pub pinned_model: Option<String>,
}

impl Companion {
  /// The name it is shown under.
  pub fn shown(&self) -> &str {
    self.label.as_deref().unwrap_or(&self.name)
  }
}

/// A turn that ended at or past the handoff mark: the session it answered in and what its runs are built from,
/// so the handoff turn after it goes out on the very session and prompt.
pub struct Due {
  pub provider: &'static dyn harness_providers::Provider,
  /// The model the turn it hands over ran on, which the handoff turn goes out on.
  pub model: String,
  pub system: String,
  pub generation: IndexMap<String, Value>,
  pub session: String,
  pub placement: Option<String>,
  /// The share of the window the turn's context reached.
  pub ratio: f64,
}

// What the handoff turn spent is the companion's own: its last answer written and its usage published.
async fn record_usage(ground: &Ground, agent: &str, reply: &Reply, run: &str) {
  if let Err(why) =
    note_answer(&ground.db, agent, &reply.mode, Some(&reply.model), reply.effort.as_deref()).await
  {
    eprintln!("companion {agent}: its last answer was not written: {}", why.detail);
  }

  let account = slot_account(&reply.mode, reply.account.as_deref());

  if let Some(spent) =
    UsageSpent::of(agent, Some(run), crate::now_ms() / 1000, &reply.spend, |_| account.clone())
  {
    ground.events.publish(spent);
  }
}

/// One handoff, written down as a run of its own, published as going among the companion's runs while it
/// runs. One its reset overtakes hands nothing over and takes back what it wrote (docs/companions.md, 「What the
/// reset cuts off」).
pub async fn hand_over(ground: &Ground, hold: &ResetHold, companion: &Companion, chat_id: i64, due: Due) {
  let began = crate::now_ms();
  let db = ground.db.clone();
  let name = companion.shown();
  let now = crate::now_ms() / 1000;
  let mut log = RunLog::new(Target {
    agent: companion.name.clone(),
    run: fill_marks(
      &COMPANION_RUNNER.runs.handoff,
      &[("chat", &chat_id.to_string()), ("at", &now.to_string())],
    ),
    chat_id: Some(chat_id),
    job_id: None,
  })
  .keeping_launch();
  let percent = (due.ratio * 100.0).round().to_string();
  let roots = harness_machine::companion_prompt::Roots {
    assets: ground.code_root(),
    data: ground.roots.data.clone(),
  };
  let took = Instant::now();
  let end = |text: String, state: &'static str| Line {
    kind: "end",
    text,
    state: Some(state),
    ..Line::default()
  };

  let instruction = match roots.agent_file("companion", "handoff.md") {
    Ok(instruction) => instruction,
    Err(why) => {
      return eprintln!("companion {} in chat {chat_id}: the handoff instruction: {why}", companion.name);
    }
  };
  let model = due.model.clone();
  let run_name = log.run().to_owned();
  let machine = harness_providers::Machine::of_process(ground.roots.data.clone());
  // A registration whose engine the arbiter stands is asked a card for the handoff's turn, as every turn on
  // it is (docs/own-llm.md, 「An engine the arbiter stands」).
  let card = match crate::cards::hold_for_model(
    ground,
    due.provider.name(),
    &model,
    &companion.name,
    Some(&due.session),
    &mut log,
  )
  .await
  {
    Ok(card) => card,
    Err(refused) => {
      log.write(&db, end(refused.detail.clone(), "failed")).await;
      return eprintln!(
        "companion {} in chat {chat_id}: the handoff failed: {}",
        companion.name, refused.detail
      );
    }
  };
  let composed = compose(
    ground,
    &machine,
    &CompanionTurn {
      agent: &companion.name,
      chat_id,
      provider: due.provider.name(),
      model: &model,
      system: &due.system,
      prompt: &instruction,
      privileged: true,
      resume: Some((&due.session, due.placement.as_deref())),
      card: card.as_ref().map(|card| card.node.as_str()),
      run: &run_name,
      answers: &[],
      generation: &due.generation,
      began,
    },
  )
  .await;

  log
    .write(
      &db,
      Line {
        kind: "start",
        text: fill_marks(&COMPANION_RUNNER.log.handoff_start, &[("name", name), ("percent", &percent)]),
        ..Line::default()
      },
    )
    .await;

  let composed = match composed {
    Ok(composed) => composed,
    Err(refused) => {
      if let Some(card) = &card {
        crate::cards::give_back(ground, card).await;
      }

      let _keeping = hold.keeping().await;

      if overtaken::overtaken(&db, &companion.name, began).await {
        return overtaken::take_back(&db, &run_name).await;
      }

      log.write(&db, end(refused.detail.clone(), "failed")).await;
      return eprintln!(
        "companion {} in chat {chat_id}: the handoff failed: {}",
        companion.name, refused.detail
      );
    }
  };

  log.launched(&db, Some(&composed.system_path), &instruction, &[]).await;
  println!(
    "companion {} in chat {chat_id}: its context reached {percent}% of the window, the session hands itself over",
    companion.name
  );

  let _going = going(&ground.events, &companion.name, &run_name, chat_id);
  let replied = run_turn(
    ground,
    &companion.name,
    crate::lines::chat::LINE,
    due.provider.name(),
    &composed,
    Some(&due.session),
    &mut log,
    true,
    None,
    None,
  )
  .await;

  if let Some(card) = &card {
    crate::cards::give_back(ground, card).await;
  }

  // What the handoff came to is kept under the hold its reset waits on, the point read again under it, as a turn's is.
  let keeping = hold.keeping().await;
  let replied = match replied {
    Ok(_) | Err(NoAnswer::Cut(_) | NoAnswer::Failed(_))
      if overtaken::overtaken(&db, &companion.name, began).await =>
    {
      Err(NoAnswer::Overtaken)
    }
    replied => replied,
  };
  let reply = match replied {
    Ok(reply) => reply,
    Err(NoAnswer::Overtaken) => {
      drop(keeping);
      overtaken::take_back(&db, &run_name).await;
      super::run::let_go_overtaken(ground, &companion.name, due.provider, &model, &composed.session).await;

      return println!("companion {} in chat {chat_id}: its reset overtook the handoff", companion.name);
    }
    Err(
      NoAnswer::Cut(reason)
      | NoAnswer::Failed(reason)
      | NoAnswer::Unsubmitted(reason)
      | NoAnswer::Errored { error: reason, .. },
    ) => {
      log.write(&db, end(reason.clone(), "failed")).await;
      return eprintln!("companion {} in chat {chat_id}: the handoff failed: {reason}", companion.name);
    }
  };
  let note = trim(&reply.text).to_owned();

  // What the handoff spent is the companion's own, and the turn is the last of the generation it closes.
  record_usage(ground, &companion.name, &reply, &run_name).await;

  if let Ok(Some(closing)) = sessions::session(&db, &companion.name, chat_id).await {
    let _ = sessions::record_turn(
      &db,
      &closing,
      &[
        Kept {
          role: "user",
          text: &instruction,
        },
        Kept {
          role: "assistant",
          text: &note,
        },
      ],
      None,
      None,
    )
    .await;
  }

  if note.is_empty() || note.to_lowercase() == COMPANION_RUNNER.no_note {
    log.write(&db, end(COMPANION_RUNNER.log.no_handoff.clone(), "failed")).await;
    return eprintln!(
      "companion {} in chat {chat_id}: the session wrote no handoff note, it goes on as it was",
      companion.name
    );
  }

  match sessions::hand_over(&db, &companion.name, chat_id, &note).await {
    Ok(opened) => {
      let seconds = (took.elapsed().as_millis() as f64 / 1000.0).round().to_string();
      let text = fill_marks(
        &COMPANION_RUNNER.log.handed_over,
        &[
          ("chars", &length(&note).to_string()),
          ("seconds", &seconds),
          ("generation", &opened.generation.to_string()),
        ],
      );

      log.write(&db, end(text, "succeeded")).await;
    }
    Err(why) => {
      log.write(&db, end(why.detail.clone(), "failed")).await;
      eprintln!("companion {} in chat {chat_id}: the handoff failed: {}", companion.name, why.detail);
    }
  }
}
