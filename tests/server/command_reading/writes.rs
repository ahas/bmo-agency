//! The reading of what a job's command or file tool writes into a place the job guard keeps: another worker's
//! checkout and the statement document.

use agency_server::command_reading::writes::{
  Checkouts, ForeignCheckout, foreign_command_write, foreign_file_write, structure_command_write,
  structure_file_write,
};

const HOME: &str = "/home/tester";

fn checkout(worker: &str, cwd: &str) -> ForeignCheckout {
  ForeignCheckout {
    worker: worker.into(),
    repository: worker.into(),
    cwd: cwd.into(),
  }
}

// The job works in its own checkout; another worker's stands beside it, and a third holds both in its
// directory.
fn checkouts() -> Checkouts {
  Checkouts {
    own: vec!["/home/tester/repos/agency".into()],
    foreign: vec![checkout("web", "/home/tester/repos/web"), checkout("umbrella", "/home/tester/repos")],
  }
}

fn foreign(command: &str) -> Option<String> {
  foreign_command_write(command, &checkouts(), HOME)
    .map(|(checkout, why)| format!("{}: {why}", checkout.worker))
}

#[test]
fn a_write_into_another_checkout_is_read_however_the_command_writes_it() {
  for command in [
    "echo hi > /home/tester/repos/web/notes.md",
    "cp a.txt ~/repos/web/",
    "sed -i 's/a/b/' $HOME/repos/web/src/main.ts",
    "git -C /home/tester/repos/web commit -m x",
    "cd /home/tester/repos/web && git add . && git commit -m x",
    "cd ~/repos/web && echo x >> notes.md",
    "python3 -c \"open('/home/tester/repos/web/a.py', 'w').write('x')\"",
    "python3 - <<'EOF'\nfrom pathlib import Path\nPath('/home/tester/repos/web/a.py').write_text('x')\nEOF",
    "bash -c 'touch /home/tester/repos/web/new.txt'",
    "D=/home/tester/repos/web; rm -f \"$D/old.txt\"",
  ] {
    assert!(foreign(command).is_some_and(|why| why.starts_with("web: ")), "let through: {command}");
  }
}

#[test]
fn a_read_of_another_checkout_and_a_write_beside_it_are_not_writes_into_it() {
  for command in [
    "cat /home/tester/repos/web/README.md",
    "rg -n route /home/tester/repos/web/src",
    "git -C /home/tester/repos/web log --oneline -5",
    "cp /home/tester/repos/web/a.txt ./a.txt",
    "echo hi > /home/tester/repos/agency/notes.md",
    "python3 -c \"print(open('/home/tester/repos/web/a.py').read())\"",
    "cd /home/tester/repos/web && cd /tmp && echo x > notes.md",
  ] {
    assert_eq!(foreign(command), None, "{command}");
  }
}

#[test]
fn a_file_tool_writes_into_the_deepest_checkout_its_path_lies_inside() {
  let known = checkouts();
  let whose = |path: &str| foreign_file_write(path, &known).map(|checkout| checkout.worker.clone());

  assert_eq!(whose("/home/tester/repos/web/src/a.ts").as_deref(), Some("web"));
  assert_eq!(whose("/home/tester/repos/web").as_deref(), Some("web"));
  assert_eq!(whose("/home/tester/repos/web-w2/src/a.ts").as_deref(), Some("umbrella"));
  assert_eq!(whose("/home/tester/repos/agency/src/a.rs"), None);
  assert_eq!(whose("/home/tester/notes/a.md"), None);
}

#[test]
fn a_command_writing_inside_a_checkout_nested_in_another_s_directory_is_that_checkout_s() {
  assert_eq!(foreign("echo x > /home/tester/repos/agency/a.md"), None);
  assert_eq!(foreign("cd ~/repos/agency && git commit -m x"), None);
  assert!(
    foreign("echo x > /home/tester/repos/STRUCTURE.md").is_some_and(|why| why.starts_with("umbrella: "))
  );
}

#[test]
fn a_change_to_the_statement_document_is_read_in_any_directory_and_a_read_of_it_is_not() {
  for command in [
    "echo x >> STRUCTURE.md",
    "sed -i 's/a/b/' docs/../STRUCTURE.md",
    "git checkout -- STRUCTURE.md",
    "mv /tmp/new.md /home/tester/repos/agency/STRUCTURE.md",
  ] {
    assert!(structure_command_write(command).is_some(), "let through: {command}");
  }

  for command in ["cat STRUCTURE.md", "rg -n Accounts STRUCTURE.md", "git diff STRUCTURE.md"] {
    assert_eq!(structure_command_write(command), None, "{command}");
  }

  assert!(structure_file_write("/home/tester/repos/agency/STRUCTURE.md").is_some());
  assert!(structure_file_write("STRUCTURE.md").is_some());
  assert!(structure_file_write("/home/tester/repos/agency/docs/STRUCTURE.md.bak").is_none());
}
