//! A runner line's configuration, and what a search run and a research branch are given as their standing
//! instructions, read off the prompts of this checkout with the site tools the web namespace lists.

use std::path::PathBuf;

use agency_server::lines::Trees;
use agency_server::lines::runner::*;
use agency_server::lines::worker::rulebook;

// The checkout's own prompts, where the instructions are read from.
fn trees() -> Trees {
  let code = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

  Trees {
    code: code.clone(),
    agents: code.join("prompts").join("agents"),
    source: code.clone(),
    data: code.clone(),
    home: code,
  }
}

// The English words the rulebooks give the run to write: the section of a product line's news, the
// three-line summary, an introduction and a conclusion.
const NEWS_SECTION: &str = "New products and prices";
const THREE_LINES: &str = "three-line summary";
const INTRODUCTION: &str = "introduction";
const CONCLUSION: &str = "conclusion";

// The catalogue as the tool host's web listing hands it to a run, a tool a line.
const LISTED: [&str; 2] = [
  "- naver_book — A book's prices. · Naver Books (search.shopping.naver.com) · since 2026-05-01 · answers search.shopping.naver.com/book/catalog/<id>",
  "- naver_blog — A blog post. · Naver Blog (blog.naver.com) · since 2026-06-01",
];

fn listed() -> String {
  LISTED.join("\n")
}

fn task() -> String {
  search_instructions(&trees(), &listed()).unwrap()
}

// What a search run has read once the section deciding a rulebook says it applies: its standing
// instructions, and the rulebook whole as the read_rulebook tool hands it back.
fn read_once_it_applies(book: &str) -> String {
  [task(), rulebook(&trees(), book).unwrap()].join("\n\n")
}

fn holds(text: &str, said: &[&str]) {
  for one in said {
    assert!(text.contains(one), "missing: {one}");
  }
}

fn lacks(text: &str, said: &[&str]) {
  for one in said {
    assert!(!text.contains(one), "present: {one}");
  }
}

fn placeholder(text: &str) -> bool {
  regex::Regex::new(r"\{\{\w+\}\}").unwrap().is_match(text)
}

#[test]
fn a_line_names_its_namespaces_alone_on_the_host_behind_the_run_s_token() {
  let server = |namespace: &str| {
    format!(
      "  \"{namespace}\": {{\n    \"type\": \"http\",\n    \"url\": \"http://127.0.0.1:3290/mcp/{namespace}\",\n    \"headers\": {{\n      \"Authorization\": \"Bearer ${{BMO_MCP_TOKEN}}\"\n    }}\n  }}"
    )
  };

  // In the order the deployment's writer writes them, which the line's prefix is read off. The toolbook is
  // the host's own face `bmo use` asks, never a server named in a line's configuration.
  assert_eq!(
    servers_on(3290, &["web", "worker"]).pretty(),
    format!("{{\n{},\n{}\n}}", server("web"), server("worker"))
  );
  assert!(!servers_on(3290, &["web", "worker"]).pretty().contains("toolbook"));
}

// is the instructions alone, every mark in them filled
#[test]
fn the_prefix_is_the_same_for_every_run_and_carries_nothing_of_one() {
  // The instructions are composed from the line alone, never from a request, so a keepalive ping on the
  // line warms exactly what the next search of any depth reads.
  let first = task();

  assert_eq!(task(), first);
  assert!(!placeholder(&first));
}

// is told to draw the done condition out of the question and stop the moment it is met
#[test]
fn is_told_to_draw_the_done_condition_out_of_the_question() {
  holds(
    &task(),
    &[
      "Before the first query, work out from the question what a finished answer to it is",
      "the question sets it, not the depth",
      "the moment it is met, stop and write the result",
      // What is still open and would not change the answer is admitted, not chased.
      "written as unconfirmed and left there",
    ],
  );
}

// is told there is no clock, and what does end the run instead
#[test]
fn is_told_there_is_no_clock() {
  holds(
    &task(),
    &[
      "A run has no time limit",
      "Slow is better than cut off",
      "The three things that end a run are the done condition being met, the branch allowance running out, and",
      "stopping the run by hand",
      "no depth turns it into looking for as long as you like",
    ],
  );
}

// names no duration at all
#[test]
fn names_no_duration_at_all() {
  lacks(
    &task(),
    &["minutes in all", "research closes after", "RESEARCH CLOSED", "co-worker gate"],
  );
}

// and says whether it was met on the result's first line, for the runner
#[test]
fn says_whether_it_was_met_on_the_results_first_line() {
  holds(
    &task(),
    &[
      "\"ANSWERED: yes :: <the done condition, in a few words>\"",
      "\"ANSWERED: no :: <what the answer still lacks>\"",
    ],
  );
}

// the task asks for one line per page at the close of every block that claims something
#[test]
fn the_task_asks_for_one_line_per_page() {
  holds(
    &task(),
    &[
      "\"SOURCE [n]: <url> :: <what that page supports>\"",
      "one line per page",
      // The page has to be one the run actually opened, so a search-result link is refused.
      "never a search-result link",
      // And the structure lines it names include this one, so it is read as structure, not markup.
      "The ANSWERED, TITLE, TOPIC, KEYWORDS, RULE, KIND, DOCUMENT, LISTEN, VERDICT, SUMMARY, SECTION, UNCONFIRMED, NAME, ADDRESS, MAP, ROADVIEW, MENU, RELEASED, NEWER, STOCK, DEAL, MORE, IMAGE, REVIEW and SOURCE lines are structure",
    ],
  );
}

// and says what to write when there is no page behind a claim
#[test]
fn says_what_to_write_when_there_is_no_page_behind_a_claim() {
  // A source that does not back the claim is worse than an admitted gap, so the gap is what gets written.
  holds(&task(), &["unconfirmed", "a page that does not back it"]);
}

// carries every rulebook or its deciding section after the task
#[test]
fn carries_every_rulebook_or_its_deciding_section() {
  let task = task();

  holds(
    &task,
    &[
      "RULEBOOK prompts/rules/search-kinds.md:",
      "RULEBOOK prompts/rules/product-search.md (sections: Whether this rulebook applies at all):",
      "RULEBOOK prompts/rules/restaurant-search.md (sections: Whether this rulebook applies at all):",
      "RULEBOOK prompts/rules/investment-search.md (sections: Whether this rulebook applies at all):",
      "read it whole with the read_rulebook tool before searching",
    ],
  );
  lacks(&task, &["## The two views", "## The shape of a product answer", "## The owner's rule"]);
  assert!(!placeholder(&task));
}

// one rulebook alone decides whether a rule applies, and says so before anything else
#[test]
fn one_rulebook_alone_decides_whether_a_rule_applies() {
  holds(
    &task(),
    &[
      "## Whether this rulebook applies at all",
      "Who asked never brings it into play",
      "No kind here sets a `RULE:` line.",
      "this line belongs to a restaurant or delivery recommendation and to nothing else",
    ],
  );
}

// names where IT news is skimmed, and holds the original as what makes it a fact
#[test]
fn names_where_it_news_is_skimmed() {
  holds(
    &task(),
    &[
      "news.hada.io",
      "GeekNews",
      "Look at the official documentation and GitHub releases first;",
      "open the original the item points at and write as fact only what the original",
    ],
  );
}

// tells the run to attach pictures where the picture is the material, and none where it is not
#[test]
fn tells_the_run_to_attach_pictures_where_the_picture_is_the_material() {
  holds(
    &task(),
    &[
      "Design, screens and anything looked at",
      "and in the body where the answer",
      "a question after one value or one fact is answered by that value, and a picture nobody asked for is noise",
      "The deployment keeps its own copy of every picture you name",
      "look at every picture you mean to name with the view_image tool",
      "the document shows lines written together as one slide gallery",
      "\"<dish> :: <price>\"",
    ],
  );
}

// tells the run to carry reviews as they were written, with their ratings and their photos
#[test]
fn tells_the_run_to_carry_reviews_as_they_were_written() {
  holds(
    &task(),
    &[
      "\"REVIEW: <the rating> :: <the review's own words> :: <the page it is on>\"",
      "quoted rather than rewritten",
      "Put the bad reviews in first",
      "write those worth showing among the entry's other pictures, in the group of their shot",
      "otherwise the caption saying they came from a review",
    ],
  );
}

// a single value is met the moment one reliable page gives it, and nothing unasked is added
#[test]
fn a_single_value_is_met_the_moment_one_reliable_page_gives_it() {
  holds(&task(), &["a question after one value or one fact", "A brief result"]);
}

// a restaurant search gives three to five places across themes, and a delivery search checks the app, the
// area, the hours, the other app's price and the discount
#[test]
fn a_restaurant_search_gives_three_to_five_places() {
  holds(
    &read_once_it_applies("restaurant-search"),
    &[
      "at least three places and at most five",
      "food themes",
      "## Delivery",
      "**Coupang Eats first.**",
      "**Every shop that delivers there.**",
      "**Open at the time asked.**",
      "its break time",
      "the hours were not confirmed",
      "Baemin",
      "**Is the discount real.**",
    ],
  );
}

// the body is the three-line summary, sections named for their content, and what went unconfirmed
#[test]
fn the_body_is_the_summary_sections_and_what_went_unconfirmed() {
  let task = task();
  let bare = format!("no {INTRODUCTION} and no {CONCLUSION}");
  let summary = format!("### The {THREE_LINES}");

  holds(
    &task,
    &[
      "\"SECTION: <what it holds>\"",
      &bare,
      "Every result has it and it stands last",
      // The three forms, with three facts side by side never the default.
      "background, fact, verdict",
      "the answer, then the two conditions that narrow it",
      "which is rare and never the default",
      "Three lines is the aim, not a quota",
      // The rulebook appended to the task is where the criteria themselves stand.
      "## The shape of the body",
      &summary,
      "**The document's nature decides the order**",
    ],
  );
  lacks(&task, &["\"INTRO:\"", "\"CONCLUSION:\""]);
}

// a product search sets its candidates on different criteria and still picks one
#[test]
fn a_product_search_sets_its_candidates_on_different_criteria() {
  let task = read_once_it_applies("product-search");

  holds(
    &task,
    &[
      "no two lines share a criterion",
      // The rulebook is where it is decided, and it decides whether it applies at all.
      "## Whether this rulebook applies at all",
      "## The shape of a product answer",
      "**The summary sets the candidates on different criteria**",
      "A product search answer still chooses one",
    ],
  );
  lacks(&task, &["no INTRO, no BODY and no CONCLUSION"]);
}

// every candidate is told to carry its release, successor, selling state and discount
#[test]
fn every_candidate_carries_its_release_successor_selling_state_and_discount() {
  holds(
    &read_once_it_applies("product-search"),
    &[
      "\"RELEASED: <when it came out>\"",
      "\"NEWER: <the successor or the newest model>\"",
      "\"STOCK: <the state> :: <when it was read> :: <the page it was read on>\"",
      "\"DEAL: <the price now> :: <the usual price> :: <whether the discount is real>\"",
      "unconfirmed",
      // The state is the page's own words, read on the page, not a verdict off a listing.
      "the product page's own words",
      "## Reading the selling state",
      "## Whether a discount is real",
    ],
  );
}

// a product search searches the line's news and writes it in a section of its own
#[test]
fn a_product_search_searches_the_lines_news() {
  let listed = format!("5. `SECTION: {NEWS_SECTION}`");

  holds(
    &task(),
    &["**Where it applies, the product line's news is searched beside the sellers**"],
  );
  holds(
    &read_once_it_applies("product-search"),
    &[
      "## The product line's news",
      &listed,
      "- **The line's news is searched first**",
      "It is filled from the line's news search, never from the seller pages",
    ],
  );
}

// a one-value search is told to leave no document
#[test]
fn a_one_value_search_is_told_to_leave_no_document() {
  holds(
    &task(),
    &[
      "\"DOCUMENT: no\"",
      "a page about it is a page nobody opens",
      "## When a search leaves no document",
    ],
  );
}

// carries the catalogue itself, so a tool made today is in front of tomorrow's search
#[test]
fn carries_the_catalogue_itself() {
  let task = task();

  for line in LISTED {
    assert!(task.contains(line), "missing: {line}");
  }

  holds(
    &task,
    &[
      "Read this list before your first query",
      "a Naver book, shopping, blog or cafe address goes to its Naver tool",
    ],
  );
  lacks(&task, &["{{tools}}"]);
}

// a branch is shown the same catalogue rather than a list of its own
#[test]
fn a_branch_is_shown_the_same_catalogue() {
  let task = branch_instructions(&trees(), &listed()).unwrap();

  for line in LISTED {
    assert!(task.contains(line), "missing: {line}");
  }

  holds(&task, &["is read with its Naver tool"]);
  lacks(&task, &["{{tools}}"]);
}

// tells the search to repair a broken tool rather than go back to searching for the value
#[test]
fn tells_the_search_to_repair_a_broken_tool() {
  // The rulebook the task is appended with says the same, since that is where the criterion is.
  holds(
    &task(),
    &[
      "a broken tool is never a reason to go back to searching for that value every time",
      "## The tools before the search",
    ],
  );
}
