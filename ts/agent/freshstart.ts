// A session handed over to a new one (docs/sessions.md, 「Session replacement」) is agency's server's; what
// stays here is the name the summariser's runs are registered under, and where the CLI keeps the
// old session's record.

export const summaryStage = "session-summary";

// Where the CLI keeps a session's records is the agent base's reading of the CLI (`cli-transcripts.ts`).
export { cliProjectsRoot, projectDir, transcriptPath } from "./cli-transcripts";

