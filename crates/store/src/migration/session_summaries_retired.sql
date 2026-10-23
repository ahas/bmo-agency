-- No session summary is kept anywhere (docs/sessions.md, 「Session replacement」): a closed session hands
-- nothing over, so neither an agent's kept summary nor a note waiting for a worker's next run stays.
DROP TABLE "session_summaries";

DELETE FROM "settings"
 WHERE starts_with("key", 'agent_session_summary:') OR starts_with("key", 'agent_session_summary_thread:');
