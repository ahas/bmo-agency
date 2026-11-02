//! The `delegate` tool's executable: one call handed on standard input, answered on standard output
//! (docs/tools.md, 「A tool's folder」).

#[tokio::main(flavor = "current_thread")]
async fn main() -> std::process::ExitCode {
  tool_call::answer_standard_io(delegate_tool::answer).await
}
