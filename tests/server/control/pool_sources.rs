//! A seat asked of the pool under its caller's own source and work key, of a running server: a runner line's
//! seat enqueued, kept, asked again and answered under its own label and key, a co-work branch's and a builder
//! turn's as before under the role's key, a reservation held under the key it names or the role's, and a
//! label that is no label refused.

use serde_json::{Value, json};

use crate::scratch::Scratch;
use crate::standing;

async fn post(port: u16, token: &str, path: &str, body: Value) -> (u16, Value) {
  let answer = reqwest::Client::new()
    .post(format!("http://127.0.0.1:{port}{path}"))
    .bearer_auth(token)
    .json(&body)
    .send()
    .await
    .unwrap();

  (answer.status().as_u16(), answer.json().await.unwrap_or(Value::Null))
}

fn seat(source: Option<&str>, key: &str, work_key: Option<&str>) -> Value {
  let mut asked = json!({
    "requestKey": key,
    "owner": "research:1",
    "role": "tracer",
    "selection": "Light",
    "tier": "Light",
    "provider": "claude-code",
    "yielding": false,
  });

  if let Some(source) = source {
    asked["source"] = json!(source);
  }

  if let Some(work_key) = work_key {
    asked["workKey"] = json!(work_key);
  }

  asked
}

#[tokio::test(flavor = "multi_thread")]
async fn a_seat_is_kept_under_its_caller_s_own_source_and_work_key() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  scratch
    .run(
      r#"INSERT INTO "coworker_pool_roles" ("name", "description", "prompt", "tools", "skills", "enabled", "pooled", "set_at")
         VALUES ('tracer', 'Traces a road', '', '', '', 1, 1, 0) ON CONFLICT DO NOTHING"#,
      vec![],
    )
    .await;
  scratch
    .run(
      r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ('role:tracer', 'Light')
         ON CONFLICT ("key") DO UPDATE SET "tier" = excluded."tier""#,
      vec![],
    )
    .await;

  scratch
    .run(
      r#"INSERT INTO "coworker_pool_tier_limits" ("provider", "tier", "capacity", "set_at") VALUES ('claude-code', 'Light', 4, 0)
         ON CONFLICT ("provider", "tier") DO UPDATE SET "capacity" = 4"#,
      vec![],
    )
    .await;

  // A runner line's seat: kept under its own label and key, and the same one asked again by its token.
  let (status, research) =
    post(port, token, "/seat-requests", seat(Some("search"), "research:1:0", Some("search"))).await;

  assert_eq!(status, 201, "{research}");
  assert_eq!(
    (&research["source"], &research["workKey"], &research["tier"], &research["turn"]),
    (&json!("search"), &json!("search"), &json!("Light"), &json!(false))
  );

  let mut again = seat(Some("search"), "research:1:0", Some("search"));

  again["token"] = research["token"].clone();

  let (status, kept) = post(port, token, "/seat-requests", again).await;

  assert_eq!((status, &kept["token"]), (201, &research["token"]), "{kept}");

  // A co-work branch's seat with no source or key named, and a builder turn's place, as before.
  let (_, cowork) = post(port, token, "/seat-requests", seat(None, "order:1:0", None)).await;

  assert_eq!(
    (&cowork["source"], &cowork["workKey"]),
    (&json!("cowork"), &json!("role:tracer")),
    "{cowork}"
  );

  let mut turn = seat(Some("turn"), "order:1:turn", None);

  turn["placeKey"] = json!("checkout:1");

  let (status, place) = post(port, token, "/seat-requests", turn).await;

  assert_eq!(status, 201, "{place}");
  assert_eq!(
    (&place["source"], &place["workKey"], &place["turn"]),
    (&json!("worker"), &json!("role:tracer"), &json!(true))
  );

  // A label that is no label is refused.
  assert_eq!(
    post(port, token, "/seat-requests", seat(Some("Not A Label"), "research:1:1", None))
      .await
      .0,
    422
  );

  // A reservation held under the key it names, and under the role's where it names none.
  let reserve = |key: &str, work_key: Option<&str>| {
    let mut asked = json!({
      "reservationKey": key, "owner": "research:1", "source": "search", "role": "tracer",
      "selection": "Light", "tier": "Light", "provider": "claude-code", "count": 1,
    });

    if let Some(work_key) = work_key {
      asked["workKey"] = json!(work_key);
    }

    asked
  };
  let (status, held) =
    post(port, token, "/reservations", reserve("research:1:seats", Some("search"))).await;

  assert_eq!(
    (status, &held["workKey"], &held["source"]),
    (201, &json!("search"), &json!("search")),
    "{held}"
  );

  let (_, plain) = post(port, token, "/reservations", reserve("research:2:seats", None)).await;

  assert_eq!(plain["workKey"], json!("role:tracer"), "{plain}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_seat_is_asked_under_an_agent_role_at_its_key_s_co_worker_tier() {
  let scratch = Scratch::new().await;
  let home = tempfile::tempdir().unwrap();
  let data = home.path().join("data");
  let token = "secret";

  std::fs::create_dir_all(&data).unwrap();

  let server = standing::stand_server(home.path(), &data, &scratch.address(), token);
  let port = server.port;

  for (role, tier) in [("aide", Some("Light")), ("chief", Some("Frontier")), ("unranked", None)] {
    scratch
      .run(
        r#"INSERT INTO "agent_roles" ("name", "label", "bundles", "position", "created_at", "updated_at")
           VALUES ($1, $1, '[]', 99, 0, 0) ON CONFLICT DO NOTHING"#,
        vec![role.into()],
      )
      .await;

    if let Some(tier) = tier {
      scratch
        .run(
          r#"INSERT INTO "model_tier_assignments" ("key", "tier") VALUES ($1, $2)
             ON CONFLICT ("key") DO UPDATE SET "tier" = excluded."tier""#,
          vec![format!("role:{role}").into(), tier.into()],
        )
        .await;
    }
  }

  scratch
    .run(
      r#"INSERT INTO "coworker_pool_tier_limits" ("provider", "tier", "capacity", "set_at") VALUES ('claude-code', 'Light', 4, 0)
         ON CONFLICT ("provider", "tier") DO UPDATE SET "capacity" = 4"#,
      vec![],
    )
    .await;

  let asked = |role: &str, key: &str| {
    json!({
      "requestKey": key, "owner": key, "role": role, "selection": "auto", "source": "delegate",
      "provider": "claude-code",
    })
  };

  // An agent role whose key names a co-worker tier lends a seat at that tier, under its own key.
  let (status, seat) = post(port, token, "/seat-requests", asked("aide", "delegate:1")).await;

  assert_eq!(status, 201, "{seat}");
  assert_eq!(
    (&seat["role"], &seat["tier"], &seat["workKey"], &seat["source"], &seat["state"]),
    (
      &json!("aide"),
      &json!("Light"),
      &json!("role:aide"),
      &json!("delegate"),
      &json!("waiting")
    )
  );

  // A tier no co-worker is borrowed at, a key with no tier, and a name no role holds are refused.
  for (role, key) in [("chief", "delegate:2"), ("unranked", "delegate:3"), ("nobody", "delegate:4")] {
    let (status, said) = post(port, token, "/seat-requests", asked(role, key)).await;

    assert_eq!(status, 422, "{role}: {said}");
  }
}
