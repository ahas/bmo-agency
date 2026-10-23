//! A turn's context as the turn route reads it from a body: every part optional, the room's block whole,
//! the road and the persona trimmed and none where blank, a work turn's parts whole by the names a job's
//! runner sends, and a body that does not read refused. A start's scope, a question's answer, a session's
//! state and a turn's end as the routes read and write them.

use std::collections::BTreeMap;

use agency_server::control::session_ops::{
  answer_read, context_read, scope_read, scope_value, state_value,
};
use agent_session::{
  AccountFailure, Answer, Apart, Asks, Context, Conversation, Failure, GateAsk, JobMark, Mapping, Outcome,
  Question, Reply, Resume, RoomBlock, RunEnding, RunParts, Scope, SessionId, Span, Spend, State,
  TurnContext, TurnEnd, WentOut, WorkParts,
};
use serde_json::json;

#[test]
fn a_gates_one_turn_reads_its_system_prompt_thinking_and_mapping() {
  assert_eq!(
    context_read(&json!({
      "gate": {
        "system": "  the gate's instructions  ",
        "thinking": false,
        "model": "a-model",
        "effort": " low ",
        "answerTokens": 2048,
      }
    }))
    .unwrap()
    .gate,
    Some(GateAsk {
      system: Some("the gate's instructions".into()),
      thinking: Some(false),
      model: Some("a-model".into()),
      effort: Some("low".into()),
      answer_tokens: Some(2048),
    })
  );
}

#[test]
fn a_turns_context_reads_every_part_it_names_and_none_of_what_it_does_not() {
  assert_eq!(context_read(&json!({ "session": "a", "prompt": "p" })).unwrap(), TurnContext::default());
  assert_eq!(
    context_read(&json!({
      "room": { "chatId": -5, "text": "the room's block", "mark": "[a]" },
      "via": " console ",
      "basePersona": "  ",
      "privileged": true,
      "answers": [7, 8],
      "began": 1_000,
    }))
    .unwrap(),
    TurnContext {
      room: Some(RoomBlock {
        chat_id: -5,
        text: "the room's block".into(),
        mark: "[a]".into()
      }),
      via: Some("console".into()),
      base_persona: None,
      privileged: true,
      answers: vec![7, 8],
      began: Some(1_000),
      run: None,
      job: None,
      work: None,
      gate: None,
    }
  );
  assert!(context_read(&json!({ "room": { "text": "no room" } })).is_err());
  assert_eq!(
    context_read(&json!({ "job": 41, "item": 1802 })).unwrap().job,
    Some(JobMark {
      job: 41,
      item: Some(1802)
    })
  );
  assert_eq!(
    context_read(&json!({ "item": 1802 })).unwrap().job,
    None,
    "an item with no job names none"
  );

  let parts = context_read(&json!({
    "run": "bmo-digest:digest:companion:-7:1",
    "parent": null,
    "agent": "companion",
    "chatId": -7,
    "step": 1,
    "steps": 2,
    "workdir": "",
    "desk": "desk-1",
    "registration": null,
  }))
  .unwrap()
  .run;

  assert_eq!(
    parts,
    Some(RunParts {
      name: "bmo-digest:digest:companion:-7:1".into(),
      parent: None,
      agent: Some("companion".into()),
      chat_id: Some(-7),
      step: Some(1),
      steps: Some(2),
      workdir: None,
      desk: Some("desk-1".into()),
      registration: None,
    })
  );
  assert_eq!(context_read(&json!({ "agent": "companion" })).unwrap().run, None);
  assert!(context_read(&json!({ "answers": "7" })).is_err());
}

// A work turn's body as a job's runner sends it, every part named.
fn work_body() -> serde_json::Value {
  json!({
    "session": "worker-a@worker",
    "prompt": "p",
    "provider": "claude-code",
    "model": "opus",
    "mapping": { "model": "opus-5", "effort": "high", "tier": "frontier", "answer_tokens": 32_000 },
    "resume": { "kind": "session", "id": "harness-1" },
    "cwd": "/checkouts/a",
    "name": "job-41",
    "title": "#1802 build",
    "job": 41,
    "item": 1802,
    "chat": -5,
    "unit": "bmo-job-41",
    "user": "7",
    "work_key": "job-worker",
    "hears_words": true,
    "variables": { "BMO_EXTRA": "1" },
  })
}

#[test]
fn a_work_turns_parts_read_whole_by_the_names_a_jobs_runner_sends() {
  let context = context_read(&work_body()).unwrap();

  assert_eq!(
    context.job,
    Some(JobMark {
      job: 41,
      item: Some(1802)
    })
  );
  assert_eq!(
    context.work,
    Some(WorkParts {
      provider: "claude-code".into(),
      model: "opus".into(),
      mapping: Mapping {
        model: "opus-5".into(),
        effort: Some("high".into()),
        tier: Some("frontier".into()),
        answer_tokens: Some(32_000),
      },
      resume: Resume::Session {
        id: "harness-1".into()
      },
      cwd: "/checkouts/a".into(),
      name: "job-41".into(),
      title: "#1802 build".into(),
      chat: -5,
      unit: Some("bmo-job-41".into()),
      user: Some("7".into()),
      work_key: "job-worker".into(),
      hears_words: true,
      variables: BTreeMap::from([("BMO_EXTRA".into(), "1".into())]),
    })
  );

  let mut least = work_body();

  for optional in ["unit", "user", "hears_words", "variables"] {
    least.as_object_mut().unwrap().remove(optional);
  }

  least["mapping"] = json!({ "model": "opus-5" });
  least["resume"] = json!({ "kind": "auto" });

  let work = context_read(&least).unwrap().work.unwrap();

  assert_eq!((work.unit, work.user), (None, None));
  assert!(!work.hears_words && work.variables.is_empty());
  assert_eq!((work.mapping.effort, work.mapping.tier, work.mapping.answer_tokens), (None, None, None));
  assert_eq!(work.resume, Resume::Auto);

  least["resume"] = json!({ "kind": "fresh" });
  assert_eq!(context_read(&least).unwrap().work.unwrap().resume, Resume::Fresh);

  for required in ["provider", "model", "resume", "cwd", "name", "title", "chat", "work_key"] {
    let mut missing = work_body();

    missing.as_object_mut().unwrap().remove(required);
    assert!(context_read(&missing).is_err(), "a work turn with no {required} is refused");
  }

  let mut unknown = work_body();

  unknown["resume"] = json!({ "kind": "latest" });
  assert!(context_read(&unknown).is_err(), "a resume of no known kind is refused");
  assert_eq!(context_read(&json!({ "mapping": null })).unwrap().work, None);
}

#[test]
fn a_thrown_away_scope_reads_its_provider_model_and_folder_and_answers_as_it_read() {
  let scope = scope_read(&json!({
    "scope": { "kind": "apart", "provider": "claude-code", "model": "sonnet", "cwd": "/places/p/01",
      "todo": 1816, "role": "scout" }
  }))
  .unwrap();
  let apart = Scope::Apart(Apart {
    provider: "claude-code".into(),
    model: "sonnet".into(),
    cwd: "/places/p/01".into(),
    todo: Some(1816),
    role: Some("scout".into()),
  });

  assert_eq!(scope, apart);
  assert_eq!(
    scope_value(&apart),
    json!({ "kind": "apart", "provider": "claude-code", "model": "sonnet", "cwd": "/places/p/01",
      "todo": 1816, "role": "scout" })
  );
  assert_eq!(
    scope_read(&json!({ "scope": { "kind": "apart", "provider": "claude-code", "model": "sonnet",
      "cwd": "/tmp/run", "todo": null } }))
    .unwrap(),
    Scope::Apart(Apart {
      provider: "claude-code".into(),
      model: "sonnet".into(),
      cwd: "/tmp/run".into(),
      todo: None,
      role: None,
    })
  );

  for missing in ["provider", "model", "cwd"] {
    let mut body = json!({ "scope": { "kind": "apart", "provider": "p", "model": "m", "cwd": "/c" } });

    body["scope"].as_object_mut().unwrap().remove(missing);
    assert!(scope_read(&body).is_err(), "a thrown-away scope with no {missing} is refused");
  }

  assert!(
    scope_read(
      &json!({ "scope": { "kind": "apart", "provider": "p", "model": "m", "cwd": "/c", "todo": "x" } })
    )
    .is_err()
  );
}

#[test]
fn a_conversations_scope_reads_its_provider_and_model_and_answers_as_it_read() {
  let conversation = Conversation {
    provider: "openrouter".into(),
    model: "deepseek/deepseek-chat".into(),
    cwd: "/places/c/01".into(),
  };
  let scope = scope_read(&json!({
    "scope": { "kind": "conversation", "provider": "openrouter",
      "model": "deepseek/deepseek-chat", "cwd": "/places/c/01" }
  }))
  .unwrap();

  assert_eq!(scope, Scope::Conversation(conversation.clone()));
  assert_eq!(
    scope_value(&scope),
    json!({ "kind": "conversation", "provider": "openrouter",
      "model": "deepseek/deepseek-chat", "cwd": "/places/c/01" })
  );
  assert_eq!(
    scope_read(&json!({ "scope": { "kind": "conversation", "provider": "codex", "model": "gpt-5" } }))
      .unwrap(),
    Scope::Conversation(Conversation {
      provider: "codex".into(),
      model: "gpt-5".into(),
      cwd: String::new(),
    }),
    "a conversation with no folder named takes the deployment's own"
  );

  for missing in ["provider", "model"] {
    let mut body = json!({ "scope": { "kind": "conversation", "provider": "p", "model": "m" } });

    body["scope"].as_object_mut().unwrap().remove(missing);
    assert!(scope_read(&body).is_err(), "a conversation with no {missing} is refused");
  }
}

#[test]
fn a_questions_answer_reads_as_allow_leave_or_a_refusal_with_its_words() {
  let read = |given: serde_json::Value| answer_read(&json!({ "answer": given }));

  assert_eq!(read(json!({ "kind": "allow" })).unwrap(), Answer::Allow);
  assert_eq!(read(json!({ "kind": "leave" })).unwrap(), Answer::Leave);
  assert_eq!(
    read(json!({ "kind": "refuse", "words": "write it as a script" })).unwrap(),
    Answer::Refuse {
      words: Some("write it as a script".into())
    }
  );
  assert_eq!(read(json!({ "kind": "refuse" })).unwrap(), Answer::Refuse { words: None });
  assert!(read(json!({ "kind": "close" })).is_err());
  assert!(answer_read(&json!({})).is_err());
}

#[test]
fn a_sessions_state_names_its_turn_its_question_and_what_it_would_continue() {
  let mut state = State::standing(
    SessionId("worker-a@worker".into()),
    "claude-code".into(),
    "opus".into(),
    "/places/worker-a".into(),
    3,
    Some(Context {
      carried: 40_000,
      window: 200_000,
    }),
  );

  assert_eq!(
    state_value(&state),
    json!({
      "session": "worker-a@worker", "provider": "claude-code", "model": "opus", "place": "/places/worker-a",
      "generation": 3, "context": { "carried": 40_000, "window": 200_000 },
      "run": null, "went_out": null, "question": null, "continues": null,
    })
  );

  state.run = Some("job-41".into());
  state.went_out = Some(WentOut {
    provider: "claude-code".into(),
    model: "opus-5".into(),
    effort: Some("high".into()),
    tier: None,
  });
  state.question = Some(Question {
    id: "q-1".into(),
    asks: Asks::Permission {
      text: "rm -f $S/*.md".into(),
      amendable: true,
    },
  });
  state.continues = Some("harness-1".into());

  let value = state_value(&state);

  assert_eq!(value["run"], "job-41");
  assert_eq!(
    value["went_out"],
    json!({ "provider": "claude-code", "model": "opus-5", "effort": "high", "tier": null })
  );
  assert_eq!(
    value["question"],
    json!({ "id": "q-1", "kind": "permission", "text": "rm -f $S/*.md", "amendable": true })
  );
  assert_eq!(value["continues"], "harness-1");
  assert_eq!(
    json!(Question {
      id: "q-2".into(),
      asks: Asks::Unknown {
        screen: "Pick one".into()
      }
    }),
    json!({ "id": "q-2", "kind": "unknown", "screen": "Pick one" })
  );
}

#[test]
fn a_turns_end_writes_its_reply_in_camel_case_and_the_rest_in_snake_case() {
  let answered = TurnEnd {
    went_out: Some(WentOut {
      provider: "claude-code".into(),
      model: "opus-5".into(),
      effort: Some("high".into()),
      tier: Some("frontier".into()),
    }),
    reply: Some(Box::new(Reply {
      mode: "claude-code".into(),
      model: Some("opus-5".into()),
      effort: Some("high".into()),
      text: "done".into(),
      session_id: Some("harness-1".into()),
      account: Some("acct-a".into()),
      context_ratio: Some(0.25),
      context_tokens: Some(50_000),
      session_usage: Some(json!({ "five_hour": 10 })),
      spend: vec![Spend {
        mode: Some("claude-code".into()),
        model: "opus-5".into(),
        tokens: 10,
        input_tokens: 4,
        output_tokens: 3,
        cache_read_tokens: 2,
        cache_write_tokens: 1,
        cost: Some(0.5),
      }],
      live_tasks: vec![],
      workflows: vec![],
      upstream: None,
      request_usages: None,
    })),
    spans: vec![Span {
      kind: "tool".into(),
      name: "Bash".into(),
      seconds: 1.5,
      times: 2,
      tokens: None,
      input_tokens: Some(4),
      output_tokens: None,
      cache_read_tokens: None,
      cache_write_tokens: None,
    }],
    compacted: true,
    ..TurnEnd::answered("job-41", "done")
  };
  let value = json!(answered);

  assert_eq!((value["outcome"].as_str(), value["answer"].as_str()), (Some("answered"), Some("done")));
  assert_eq!(value["reply"]["sessionId"], "harness-1");
  assert_eq!(value["reply"]["contextTokens"], 50_000);
  assert_eq!(value["reply"]["spend"][0]["cacheReadTokens"], 2);
  assert_eq!(value["went_out"]["tier"], "frontier");
  assert_eq!(value["spans"][0]["input_tokens"], 4);
  assert_eq!(value["compacted"], true);

  let failure = |failure: Failure| {
    json!(TurnEnd {
      failure: Some(failure),
      ..TurnEnd::of("r", Outcome::Failed)
    })["failure"]
      .clone()
  };

  assert_eq!(
    failure(Failure::Limit {
      resets_at: Some(1_700),
      account: Some("acct-a".into()),
      silenced: false
    }),
    json!({ "kind": "limit", "resets_at": 1_700, "account": "acct-a", "silenced": false })
  );
  assert_eq!(
    failure(Failure::Account {
      failure: AccountFailure::NodeToken,
      detail: "gone".into()
    }),
    json!({ "kind": "account", "failure": "node_token", "detail": "gone" })
  );
  assert_eq!(
    failure(Failure::Died {
      ending: RunEnding::Exit { code: 137 },
      said: None,
      message: "killed".into()
    }),
    json!({ "kind": "died", "ending": { "kind": "exit", "code": 137 }, "said": null, "message": "killed" })
  );
  assert_eq!(failure(Failure::Person), json!({ "kind": "person" }));

  let reprepared = TurnEnd {
    continues: Some("harness-2".into()),
    ..TurnEnd::of("job-41", Outcome::Reprepared)
  };

  assert_eq!(
    (json!(reprepared)["outcome"].as_str(), json!(reprepared)["continues"].as_str()),
    (Some("reprepared"), Some("harness-2"))
  );
  assert_eq!(json!(TurnEnd::of("r", Outcome::Blocked))["delivery"], "blocked");
}
