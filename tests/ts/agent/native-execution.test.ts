import { expect, test } from "bun:test";
import { Archive } from "@agency/archive/archive";
import { beginExecution } from "@agency/agent/execution-identity";
import { NativeExecutions } from "@agency/agent/native-execution";
import { scratchArchive, scratchDatabase } from "@agency/test-support/store";

async function fixture(provider = "claude-code") {
  const archive = await scratchArchive();
  const parent = await beginExecution(archive, provider, "worker", {});
  return { archive, tracker: new NativeExecutions(archive, parent) };
}
test("tool return and collector end do not invent successful execution", async () => {
  const { tracker } = await fixture();
  await tracker.observe({ message: { content: [{ type: "tool_use", name: "Agent", id: "a" }] } });
  expect(tracker.children.get("a")?.nativeState).toBe("requested");
  await tracker.observe({ message: { content: [{ type: "tool_result", tool_use_id: "a", content: "background launched" }] } });
  await tracker.finish();
  expect(tracker.children.get("a")).toMatchObject({ nativeState: "unresolved", endedAt: null });
});
test("explicit Claude start and terminal notification associate task with request", async () => {
  const { tracker } = await fixture();
  await tracker.observe({ type: "system", subtype: "task_started", task_type: "local_agent", task_id: "task", tool_use_id: "call" });
  expect(tracker.children.get("call")?.nativeState).toBe("started");
  await tracker.observe({ type: "system", subtype: "task_notification", task_id: "task", status: "failed" });
  await tracker.finish();
  expect(tracker.children.get("call")).toMatchObject({ nativeState: "failed", exitCode: 1, sharedPane: true });
});
test("Claude transcript structured results distinguish synchronous completion and background launch", async () => {
  const { tracker } = await fixture();
  for (const id of ["sync", "async"]) await tracker.observe({ message: { content: [{ type: "tool_use", name: "Agent", id }] } });
  const result = (id: string, status: string) => ({ toolUseResult: { agentId: id, status }, message: { content: [{ type: "tool_result", tool_use_id: id }] } });
  await tracker.observe(result("sync", "completed"));
  await tracker.observe(result("async", "async_launched"));
  expect(tracker.children.get("async")?.nativeState).toBe("started");
  await tracker.finish();
  expect(tracker.children.get("sync")).toMatchObject({ nativeState: "completed", exitCode: 0 });
  expect(tracker.children.get("async")).toMatchObject({ nativeState: "unresolved", endedAt: null });
});
test("Codex collaboration completion does not complete pending receivers; terminal evidence survives replay", async () => {
  const { tracker } = await fixture("codex");
  const event = (status: string) => ({ type: "item.completed", item: { type: "collab_agent_tool_call", receiver_thread_ids: ["child"], agents_states: { child: { status } } } });
  await tracker.observe(event("pending_init"));
  expect(tracker.children.get("child")?.nativeState).toBe("requested");
  await tracker.observe(event("running"));
  await tracker.observe(event("completed"));
  await tracker.observe(event("pending_init"));
  await tracker.finish();
  expect(tracker.children.get("child")).toMatchObject({ nativeState: "completed", exitCode: 0, sessionId: "child" });
});
