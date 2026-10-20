//! The harness plugin's tools: each one route, called through this client.

/// One tool of the agency's MCP server.
pub struct Tool {
  /// Its name, `<unit>_<what>` in snake_case.
  pub name: &'static str,
  /// What it does, for the harness's own reading.
  pub description: &'static str,
  /// The schema of its input.
  pub input: &'static str,
  /// The schema of its output.
  pub output: &'static str,
  /// The route it calls.
  pub route: &'static str,
}

/// The tools the agency's MCP server lists.
pub const TOOLS: [Tool; 2] = [
  Tool {
    name: "assign_worker",
    description: "Assign a worker to an item for the harness holding it: the agency picks the worker from the item's content and the assignee it names, and holds it until the job starts or the hold runs out.",
    input: "AssignWorker",
    output: "Assignment",
    route: "POST /assignments",
  },
  Tool {
    name: "run_job",
    description: "Run an item on its assigned worker: the harness hands over the item's content and the lease it holds, and the agency launches the worker's session on it. The agency calls nothing of the job service.",
    input: "RunJob",
    output: "Job",
    route: "POST /jobs",
  },
];
