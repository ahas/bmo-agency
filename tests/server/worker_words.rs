//! The words said to a worker, offered on a road standing in for the worker's session: taken into the turn
//! a job of the worker's is working, carried in a turn of their own where none works with the words said
//! meanwhile handed into it and the answer under the last of them, held by a pane waiting on a person with
//! that said under them once, worded by the consumer where it words them; and said across the boundary to a
//! running server, which writes them down and refuses an empty text or a name no worker holds.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use agency_server::Service;
use agency_server::config::Config;
use agency_server::door::record::Reply;
use agency_server::door::turn::{AsideOutcome, Watch};
use agency_server::places::{AgentRow, Ground};
use agency_server::seeds::deployment;
use agency_server::worker_session::{Missed, Turned};
use agency_server::worker_words::{Delivery, Way, WordRoad, WordTurn, Words, WorkerWordsHook, said};
use agency_server::{Refusal, now_ms};
use agent_session::{Aside, Fault};
use harness_providers::Machine;
use policy::worker_words::WORKER_WORDS;
use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

const WORKER: &str = "helper";

// What a turn of the words comes to on the stand-in road.
#[derive(Clone)]
enum Came {
  Answered(&'static str),
  Missed(Missed),
}

// A road standing in for the worker's session: what a push into its working turn comes to, what a turn of
// the words comes to, and a word said while that turn answers; what was pushed and what each turn was told.
struct Road {
  ground: Arc<Ground>,
  aside: Aside,
  came: Came,
  meanwhile: Option<&'static str>,
  pushed: Mutex<Vec<String>>,
  told: Mutex<Vec<String>>,
}

impl Road {
  fn new(ground: &Arc<Ground>, aside: Aside, came: Came) -> Self {
    Self {
      ground: ground.clone(),
      aside,
      came,
      meanwhile: None,
      pushed: Mutex::default(),
      told: Mutex::default(),
    }
  }
}

impl WordRoad for Road {
  async fn push(&self, _worker: &str, words: &str) -> Result<Aside, Fault> {
    self.pushed.lock().unwrap().push(words.to_owned());

    Ok(self.aside)
  }

  async fn turn<W: Watch + Send + Sync>(
    &self,
    _worker: &AgentRow,
    asked: WordTurn,
    watch: &mut W,
  ) -> Result<Option<Turned>, Refusal> {
    self.told.lock().unwrap().push(asked.prompt);

    if let Some(more) = self.meanwhile {
      said::say(&self.ground.db, WORKER, more).await?;

      let words = watch.words().await.expect("the words said meanwhile are offered to the working turn");

      self.told.lock().unwrap().push(words);
      watch.said(AsideOutcome::Delivered).await;
    }

    Ok(Some(match self.came.clone() {
      Came::Answered(text) => Turned {
        reached: true,
        reply: Some(Reply {
          text: text.to_owned(),
          ..Reply::default()
        }),
        missed: None,
      },
      Came::Missed(missed) => Turned {
        reached: false,
        reply: None,
        missed: Some(missed),
      },
    }))
  }
}

async fn ground(scratch: &Scratch, home: &std::path::Path) -> Arc<Ground> {
  let machine = Machine::with_env(
    [
      ("HOME".to_owned(), home.to_string_lossy().into_owned()),
      ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
    ],
    home.join("data"),
  );

  store::seed::settle(scratch.db(), &deployment(&machine)).await.unwrap();
  scratch
    .run(r#"INSERT INTO "agents" ("name", "kind") VALUES ($1, 'worker')"#, vec![WORKER.into()])
    .await;

  let variables: HashMap<String, String> = HashMap::new();
  let service =
    Service::new(scratch.db().clone(), Config::read(|name| variables.get(name).cloned()).unwrap());

  Arc::new(Ground::new(&service, &machine, None))
}

// The lines of a word's run in the worker's log, each its kind, name and text.
async fn lines(scratch: &Scratch, word: i64) -> Vec<(String, Option<String>, String)> {
  let rows = store::all(
    scratch.db(),
    r#"SELECT "kind", "name", "text" FROM "agent_log" WHERE "agent" = $1 AND "run" = $2 ORDER BY "id""#,
    vec![WORKER.into(), said::run_of(WORKER, word).into()],
  )
  .await
  .unwrap();

  rows
    .iter()
    .map(|row| {
      (
        row.try_get::<String>("", "kind").unwrap(),
        row.try_get::<Option<String>>("", "name").unwrap(),
        row.try_get::<String>("", "text").unwrap(),
      )
    })
    .collect()
}

async fn waiting(scratch: &Scratch) -> i64 {
  scratch
    .count(
      r#"SELECT count(*) FROM "worker_words" WHERE "worker" = $1 AND "delivered_at" IS NULL"#,
      vec![WORKER.into()],
    )
    .await
}

#[tokio::test]
async fn a_worker_s_working_job_takes_the_words_into_its_turn() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path()).await;
  let road = Arc::new(Road::new(&ground, Aside::Delivered, Came::Answered("unused")));
  let words = Words::new(ground.clone(), road.clone());
  let word = said::say(&ground.db, WORKER, "check the deploy").await.unwrap();

  assert_eq!(words.offer(WORKER, false).await.unwrap(), Delivery::Taken);
  assert_eq!(*road.pushed.lock().unwrap(), ["check the deploy"]);
  assert!(road.told.lock().unwrap().is_empty(), "no turn of their own goes over a working job");
  assert_eq!(waiting(&scratch).await, 0);
  assert_eq!(
    lines(&scratch, word).await,
    [
      ("prompt".to_owned(), Some("word".to_owned()), "check the deploy".to_owned()),
      ("note".to_owned(), None, WORKER_WORDS.held.in_job.clone()),
    ]
  );
  assert_eq!(words.offer(WORKER, false).await.unwrap(), Delivery::None);
}

#[tokio::test]
async fn an_idle_worker_answers_the_words_in_a_turn_of_their_own_with_those_said_meanwhile() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path()).await;
  let mut road = Road::new(&ground, Aside::Refused, Came::Answered("Deployed, and the logs are clean."));

  road.meanwhile = Some("and the logs");

  let road = Arc::new(road);
  let words = Words::new(ground.clone(), road.clone());
  let first = said::say(&ground.db, WORKER, "is it deployed?").await.unwrap();

  assert_eq!(words.offer(WORKER, false).await.unwrap(), Delivery::Answered);
  assert_eq!(*road.told.lock().unwrap(), ["is it deployed?", "and the logs"]);
  assert_eq!(waiting(&scratch).await, 0, "the words taken in the turn are marked as taken");

  let last = first + 1;

  assert_eq!(
    lines(&scratch, last).await.last().unwrap(),
    &("text".to_owned(), None, "Deployed, and the logs are clean.".to_owned())
  );
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "run_clocks" WHERE "agent" = $1 AND "bound_ms" = $2 AND "ended_at" IS NOT NULL"#,
        vec![WORKER.into(), ((WORKER_WORDS.answer_minutes * 60_000) as i64).into()],
      )
      .await,
    1,
    "the word's turn ran on its clock, closed as it ended"
  );
}

#[tokio::test]
async fn words_held_by_a_person_wait_with_that_said_under_them_once() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path()).await;
  let road = Arc::new(Road::new(&ground, Aside::Refused, Came::Missed(Missed::Person)));
  let words = Words::new(ground.clone(), road.clone());
  let word = said::say(&ground.db, WORKER, "one more thing").await.unwrap();

  assert_eq!(words.offer(WORKER, false).await.unwrap(), Delivery::Unreached);
  assert_eq!(
    words.offer(WORKER, true).await.unwrap(),
    Delivery::Held,
    "a round lets the pane be a while"
  );
  assert_eq!(words.offer(WORKER, false).await.unwrap(), Delivery::Unreached);
  assert_eq!(waiting(&scratch).await, 1);
  assert_eq!(
    lines(&scratch, word)
      .await
      .iter()
      .filter(|line| line.2 == WORKER_WORDS.held.person)
      .count(),
    1
  );
}

#[tokio::test]
async fn the_consumer_words_what_the_worker_is_told() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let ground = ground(&scratch, home.path()).await;
  let road = Arc::new(Road::new(&ground, Aside::Refused, Came::Answered("Done.")));
  let words = Words::new(ground.clone(), road.clone());
  let before = now_ms();

  ground.hooks.register::<WorkerWordsHook, _, _>(move |told| async move {
    let said = &told.words[0];

    (told.way == Way::Task && said.said_at >= before / 1000 * 1000)
      .then(|| format!("The owner said to {}: {}", told.worker, said.text))
  });
  said::say(&ground.db, WORKER, "hello").await.unwrap();

  assert_eq!(words.offer(WORKER, false).await.unwrap(), Delivery::Answered);
  assert_eq!(*road.told.lock().unwrap(), ["The owner said to helper: hello"]);
  assert_eq!(
    *road.pushed.lock().unwrap(),
    ["hello"],
    "an aside the consumer words none of goes as said"
  );
}

async fn post(port: u16, token: &str, path: &str, body: Value) -> u16 {
  reqwest::Client::new()
    .post(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .unwrap()
    .status()
    .as_u16()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_server_writes_down_the_words_said_to_a_worker() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();
  scratch
    .run(
      r#"INSERT INTO "agents" ("name", "kind") VALUES ($1, 'worker'), ('chat-agent', 'chat')"#,
      vec![WORKER.into()],
    )
    .await;

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  assert_eq!(post(port, token, "/workers/helper/words", json!({ "text": "  hello  " })).await, 204);
  assert_eq!(
    scratch
      .count(
        r#"SELECT count(*) FROM "worker_words" w JOIN "agent_log" l ON l."run" = $2 || '-word-helper-' || w."id"
           WHERE w."worker" = $1 AND w."text" = 'hello' AND l."kind" = 'prompt' AND l."name" = 'word'
             AND l."text" = 'hello'"#,
        vec![WORKER.into(), policy::deployment::DEPLOYMENT.agent_key.clone().into()],
      )
      .await,
    1
  );
  assert_eq!(post(port, token, "/workers/helper/words", json!({ "text": " " })).await, 400);
  assert_eq!(post(port, token, "/workers/chat-agent/words", json!({ "text": "hi" })).await, 400);
  assert_eq!(post(port, token, "/workers/nobody/words", json!({ "text": "hi" })).await, 400);
}
