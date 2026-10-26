//! Every research result leaves its document (docs/research-runs.md, 「One run, step by step」): the searcher's
//! structure lines made into a document written for its reader — headed sections in the question's language,
//! the sources as links last, pictures kept beside it, the narration lifted out — filed in bmo-artifact and
//! shared, and the room told the answer in brief with the one link, never the grounds.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use agency_server::lines::Trees;
use agency_server::research::{RESEARCH_ACTOR, brief_of, file_result, linked_instruction};
use agency_server::research_document::{Words, draw, filing, language_of, words};
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{StatusCode, Uri};
use axum::response::IntoResponse;
use serde_json::{Value, json};
use tokio::net::TcpListener;

fn trees() -> Trees {
  let code = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

  Trees {
    code: code.clone(),
    agents: code.join("prompts").join("agents"),
    source: code.clone(),
    data: code.clone(),
    home: code,
  }
}

fn english() -> Words {
  words(&trees(), "en").unwrap()
}

const BRIEF: &str = "ANSWERED: yes :: the base rate\nTITLE: The base rate stays at 3.00%\nTOPIC: the base rate today\n\
KEYWORDS: base rate, central bank\n\nSUMMARY:\n1. The base rate is 3.00% a year as of 2026-10-26 [1].\n\nSECTION: Grounds\n\
The bank's own history lists no change after 2026-08-27 [1], and a report says the board held it [2\u{2020}].\n\
SOURCE [1]: https://example.org/rates :: the bank's rate history\nSOURCE [2\u{2020}]: https://news.example/held :: snippet: held\n\n\
UNCONFIRMED:\n1. The board's own statement was not opened.";

const DEEP: &str = "ANSWERED: yes :: where to eat\nTITLE: Two noodle places near the station\nTOPIC: noodles near the station\n\
KEYWORDS: noodles\nRULE: the owner's rule\n\nLISTEN:\nA script for the ear.\nIt runs to the verdict.\n\nVERDICT:\n\
1. Go to the first place.\n\nSUMMARY:\n1. The first is cheaper [1].\n2. The second is closer [2].\n\nSECTION: Compared\n\
| Place | Price |\n|---|---|\n| First | 9,000 [1] |\n\n===ITEM===\nNAME: The first place\nADDRESS: 1 Station Road\n\
MAP: https://map.example/1\nIMAGE: https://img.example/1.jpg :: the storefront :: The first place :: outside :: official\n\
MENU: menu board photo [1]\nMeals\nNoodles :: 9,000\nDumplings :: 6,000 :: six pieces\nREVIEW: 5 :: Rich broth. :: https://reviews.example/1\n\
SOURCE [1]: https://place.example/1 :: the first place's page\n\n===ITEM===\nNAME: The second place\n\
SOURCE [2]: https://place.example/2 :: the second place's page\n\nUNCONFIRMED:\n1. Opening hours on holidays.";

#[test]
fn a_question_is_written_back_in_its_own_language_and_each_language_has_its_words() {
  assert_eq!(language_of("\u{d658}\u{c728} \u{c5bc}\u{b9c8}?"), "ko");
  assert_eq!(language_of("What is the rate?"), "en");

  let korean = words(&trees(), "ko").unwrap();

  assert!(!korean.answer.is_empty() && korean.answer != english().answer);
  assert_eq!(
    words(&trees(), "xx").unwrap(),
    english(),
    "a language with no words of its own is English"
  );
}

#[test]
fn a_brief_result_is_its_answer_its_grounds_what_was_not_confirmed_and_its_sources_and_no_structure_word() {
  let words = english();
  let drawn = draw(BRIEF, &words);

  assert_eq!(drawn.title.as_deref(), Some("The base rate stays at 3.00%"));
  assert_eq!(drawn.description.as_deref(), Some("the base rate today"));
  assert_eq!(drawn.keywords.as_deref(), Some("base rate, central bank"));
  assert!(!drawn.learning);

  let body = &drawn.body;
  let headings: Vec<&str> = body.lines().filter(|line| line.starts_with("## ")).collect();

  assert_eq!(
    headings,
    [
      format!("## {}", words.brief_answer),
      format!("## {}", words.grounds),
      format!("## {}", words.unconfirmed),
      format!("## {}", words.sources)
    ]
  );
  assert!(body.starts_with(&format!("## {}", words.brief_answer)), "{body}");
  assert!(body.contains("- [1] [the bank's rate history](https://example.org/rates)"), "{body}");
  assert!(body.contains("- [2\u{2020}] [snippet: held](https://news.example/held)"), "{body}");

  for word in [
    "ANSWERED",
    "TITLE:",
    "TOPIC:",
    "KEYWORDS:",
    "SUMMARY:",
    "SECTION:",
    "UNCONFIRMED:",
    "SOURCE",
  ] {
    assert!(!body.contains(word), "{word} in {body}");
  }
}

#[test]
fn a_deep_result_keeps_its_table_its_entries_their_menu_pictures_and_reviews_and_lifts_its_narration_out() {
  let words = english();
  let drawn = draw(DEEP, &words);
  let body = &drawn.body;

  assert_eq!(drawn.rule.as_deref(), Some("the owner's rule"));
  assert_eq!(drawn.narration.as_deref(), Some("A script for the ear.\nIt runs to the verdict."));
  assert!(!body.contains("A script for the ear"), "{body}");

  for heading in [
    &words.answer,
    &words.summary,
    &"Compared".to_owned(),
    &words.unconfirmed,
    &words.sources,
  ] {
    assert!(body.contains(&format!("## {heading}")), "{heading} in {body}");
  }

  assert!(body.contains("| First | 9,000 [1] |"));
  assert!(body.contains("### The first place") && body.contains("### The second place"));
  assert!(body.contains(&format!("- {}: 1 Station Road", words.address)));
  assert!(body.contains(&format!("- [{}](https://map.example/1)", words.map)));
  assert!(body.contains(&format!("#### {}", words.menu)) && body.contains("_menu board photo [1]_"));
  assert!(body.contains("- **Meals**") && body.contains("- Noodles \u{2014} 9,000"));
  assert!(body.contains("- Dumplings \u{2014} 6,000 (six pieces)"));
  assert!(body.contains("> Rich broth."));
  assert!(
    body.contains("![the storefront \u{b7} The first place \u{b7} outside \u{b7} official](picture-01)")
  );
  assert_eq!(drawn.pictures.len(), 1);
  assert_eq!(drawn.pictures[0].url, "https://img.example/1.jpg");
  assert_eq!(body.matches("](https://place.example/").count(), 2, "each source once, as a link");

  for word in [
    "===ITEM===",
    "NAME:",
    "ADDRESS:",
    "MAP:",
    "MENU:",
    "IMAGE:",
    "REVIEW:",
    "VERDICT:",
    "LISTEN:",
  ] {
    assert!(!body.contains(word), "{word} in {body}");
  }
}

#[test]
fn a_result_is_filed_titled_by_its_head_and_labelled_with_its_run_and_a_study_material_as_learning() {
  let drawn = draw(BRIEF, &english());
  let document = filing(&drawn, 7, "search:basic", " the rate ", Some(5));

  assert_eq!(document["kind"], "search");
  assert_eq!(document["description"], "the base rate today");
  assert_eq!(document["title"], "The base rate stays at 3.00%");
  assert_eq!(document["body"], drawn.body);
  assert_eq!(document["scope"], "5");
  assert_eq!(document["actorName"], RESEARCH_ACTOR);
  assert_eq!(document["labels"]["research"], "7");
  assert_eq!(document["labels"]["key"], "search:basic");
  assert_eq!(document["labels"]["query"], "the rate");
  assert_eq!(document["labels"]["keywords"], "base rate, central bank");

  let learning = filing(
    &draw("KIND: learning\nTITLE: A book\n\n# Part one", &english()),
    8,
    "search:thorough",
    "teach",
    None,
  );

  assert_eq!(learning["kind"], "learning");
  assert_eq!(learning["scope"], Value::Null);
  assert_eq!(filing(&draw("no head", &english()), 9, "browse", "the task", None)["title"], "the task");
}

#[test]
fn a_brief_result_gives_its_summary_and_a_deep_one_its_verdict() {
  assert_eq!(
    brief_of(BRIEF).as_deref(),
    Some("1. The base rate is 3.00% a year as of 2026-10-26 [1].")
  );
  assert_eq!(brief_of(DEEP).as_deref(), Some("1. Go to the first place."));
}

#[test]
fn the_room_is_told_the_answer_in_brief_and_one_link_and_no_grounds() {
  let told =
    linked_instruction(7, "search:basic", "the rate", Some(5), "1. 3.00%", "https://share.example/t");
  let order = told["said"]["order"].as_str().unwrap();

  assert_eq!(order.matches("https://").count(), 1, "{order}");
  assert!(
    order.contains("one or two short lines") && order.contains("no grounds, sources or document text")
  );
  assert_eq!(told["room"], 5);
}

type Calls = Arc<Mutex<Vec<(String, String, Value)>>>;

async fn artifact(
  State(calls): State<Calls>,
  method: axum::http::Method,
  uri: Uri,
  body: Bytes,
) -> impl IntoResponse {
  calls.lock().unwrap().push((
    method.to_string(),
    uri.path().to_owned(),
    serde_json::from_slice(&body).unwrap_or(Value::Null),
  ));

  match uri.path() {
    "/documents" => (StatusCode::CREATED, axum::Json(json!({ "id": 41 }))),
    "/shares" => (
      StatusCode::CREATED,
      axum::Json(json!({ "documentId": 41, "url": "https://share.example/t" })),
    ),
    _ => (StatusCode::OK, axum::Json(json!({ "id": 41 }))),
  }
}

#[tokio::test]
async fn a_picture_that_cannot_be_kept_is_drawn_from_its_address_and_the_document_shared() {
  let calls: Calls = Arc::default();
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = format!("http://{}", listener.local_addr().unwrap());
  let app = axum::Router::new().fallback(artifact).with_state(calls.clone());

  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

  let vars: BTreeMap<String, String> = [
    ("BMO_ARTIFACT_URL".to_owned(), address),
    ("BMO_ARTIFACT_TOKEN".to_owned(), "artifact-token".to_owned()),
  ]
  .into();
  // A picture on this machine is never fetched, so it stands by its address.
  let drawn = draw(
    "TITLE: A place\n\nSUMMARY:\n1. It\nIMAGE: http://127.0.0.1:1/p.jpg :: a picture",
    &english(),
  );
  let document = filing(&drawn, 7, "search:basic", "the place", Some(5));

  assert_eq!(
    file_result(&vars, &document, &drawn).await.unwrap().as_deref(),
    Some("https://share.example/t")
  );

  let made = calls.lock().unwrap().clone();
  let paths: Vec<(&str, &str)> =
    made.iter().map(|(method, path, _)| (method.as_str(), path.as_str())).collect();

  assert_eq!(paths, [("POST", "/documents"), ("PATCH", "/documents/41"), ("POST", "/shares")]);

  let ends = made[2].2["expiresAt"].as_i64().unwrap();
  let day = agency_server::research::SHARE_LIFE.as_millis() as i64;

  assert!(
    (ends - agency_server::now_ms() - day).abs() < 60_000,
    "the link ends a day after it is made"
  );
  assert!(made[1].2["body"].as_str().unwrap().contains("![a picture](http://127.0.0.1:1/p.jpg)"));
  assert_eq!(file_result(&BTreeMap::new(), &document, &drawn).await.unwrap(), None);
}

#[test]
fn the_product_line_s_news_section_stands_under_its_heading_in_the_question_s_language() {
  let words = words(&trees(), "ko").unwrap();
  let drawn = draw("TITLE: t\n\nVERDICT:\n1. a\n\nSECTION: New products and prices\nnews", &words);

  assert!(drawn.body.contains(&format!("## {}", words.news)), "{}", drawn.body);
  assert!(!drawn.body.contains("New products and prices"));
}
