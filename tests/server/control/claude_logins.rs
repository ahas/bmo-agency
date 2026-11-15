//! A Claude account signed in from the console (docs/cache-fallback.md, 「Signing an account in from the
//! console」): the sign-in's address read out of what the CLI prints, its hyperlink escapes and all; a refusal
//! said in the CLI's last words with the code taken out; and a new account's directory named by its name.

use std::path::Path;

use agency_server::control::claude_logins::{last_words, new_home, sign_in_address};
use policy::control_words::CONTROL_WORDS;

// What `claude auth login` prints with no browser to open: the address as a terminal hyperlink, its escapes
// around it, then the prompt for the code.
fn printed(address: &str) -> String {
  format!(
    "Opening browser to sign in…\nIf the browser didn't open, visit: \u{1b}]8;;{address}\u{7}{address}\u{1b}]8;;\u{7}\nPaste code here if prompted > "
  )
}

#[test]
fn the_address_is_read_out_of_the_hyperlink_the_cli_prints() {
  let address = "https://example.com/oauth/authorize?code=true&client_id=x&state=y";
  assert_eq!(sign_in_address(&printed(address)).as_deref(), Some(address));
  assert_eq!(sign_in_address("Opening browser to sign in…\n"), None);
  assert_eq!(sign_in_address("visit: http://example.com/plain"), None);
}

#[test]
fn a_refusal_says_the_last_words_with_the_code_taken_out() {
  let said = format!(
    "{}abc123-secret\n\u{1b}[31mOAuth error: invalid code abc123-secret\u{1b}[0m\n\n",
    printed("https://example.com/a")
  );
  let words = last_words(&said, Some("abc123-secret"));
  assert_eq!(words, "OAuth error: invalid code …");
  assert_eq!(last_words("", None), CONTROL_WORDS.logins.silent);
}

#[test]
fn a_new_account_s_directory_is_named_by_its_name_under_the_env_directory() {
  let root = Path::new("/data");
  assert_eq!(new_home(root, "claude-03"), Path::new("/data/env/claude-03"));
  assert_eq!(new_home(root, " Work Login! "), Path::new("/data/env/work-login"));
  assert_eq!(new_home(root, "../.."), Path::new("/data/env/"));
}
