//! The `read_rulebook` tool's executable: one call handed by the tool host on standard input, answered on
//! standard output (docs/tools.md, 「A tool's folder」).

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
  tool_call::answer_standard_io(read_rulebook_tool::answer).await
}
