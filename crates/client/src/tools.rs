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

/// What `stop_job` does, as its server and the document both say it.
pub const STOP_JOB: &str = "Stop a running job on the owner's word, by its job id or by its item, with a short reason that becomes the job's report: only a turn whose every message is the owner's stops one. The job's turn is cut short first, and the answer is the job once its turn has ended, stopped; a turn still going after the wait is refused stop-pending, and the job ends stopped when it does.";

/// The tools the agency's MCP server lists.
pub const TOOLS: [Tool; 5] = [
  Tool {
    name: "file_item",
    description: "File an item in the job service: the room it was asked in and whether the owner asked for it are the turn's, which the agency reads; an item the owner asked for himself opens at once, any other waits as a draft for his approval.",
    input: "FileItem",
    output: "ItemFiled",
    route: "POST /item-filings",
  },
  Tool {
    name: "stop_job",
    description: STOP_JOB,
    input: "StopJob",
    output: "Job",
    route: "POST /job-stops",
  },
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
  Tool {
    name: "run_research",
    description: "Send a web research out on a research key: a search at a depth, or a browse. Answered at once with the run and the road it went out on; its result is queued for the room as the run ends.",
    input: "ResearchRunAsk",
    output: "ResearchRun",
    route: "POST /research-runs",
  },
];
