import "@agency/test-support/setup";

import { describe, expect, test } from "bun:test";
import { join } from "node:path";

// The hook runs inside the gateway's LiteLLM, which the machine's own Python does not carry. The base
// class it extends is stood in for, and so is the engine instance: a server on a thread that answers a
// restore of `role-chat.bin` and refuses every other file with a 404, keeping each restore it is asked.
// Every case goes through the hook in turn, in one interpreter, and the restores each one made are told
// beside it.
const driver = `
import asyncio, http.server, importlib.util, json, sys, threading, types

custom_logger = types.ModuleType("litellm.integrations.custom_logger")
custom_logger.CustomLogger = type("CustomLogger", (), {})
for name in ("litellm", "litellm.integrations"):
    sys.modules[name] = types.ModuleType(name)
sys.modules["litellm.integrations.custom_logger"] = custom_logger

spec = importlib.util.spec_from_file_location("roles", sys.argv[1])
roles = importlib.util.module_from_spec(spec)
spec.loader.exec_module(roles)

asked = []

class Instance(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        asked.append({"path": self.path, "file": body.get("filename"), "key": self.headers.get("Authorization")})
        code = 200 if body.get("filename") == "role-chat.bin" else 404
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b"{}")

    def log_message(self, *args):
        pass

server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Instance)
threading.Thread(target=server.serve_forever, daemon=True).start()
root = f"http://127.0.0.1:{server.server_address[1]}"

async def run(cases):
    told = []
    for case in cases:
        kwargs = {"api_base": root + case.get("path", "/v1"), "api_key": "node-key", "model_info": case.get("info")}
        if case.get("role") is not None:
            kwargs["proxy_server_request"] = {"headers": {case.get("header", "x-bmo-role"): case["role"]}}
        before = len(asked)
        answer = await roles.handler.async_pre_call_deployment_hook(kwargs, "acompletion")
        told.append({"answer": answer, "asked": asked[before:]})
    return told

print(json.dumps(asyncio.run(run(json.load(sys.stdin)))))
`;

type Case = { role?: string; header?: string; info?: Record<string, unknown> | null; path?: string };
type Told = { answer: unknown; asked: { path: string; file: string; key: string }[] };

async function hook(cases: Case[]): Promise<Told[]> {
  const child = Bun.spawn(["python3", "-c", driver, join(import.meta.dir, "../../../crates/llm-gateway/hooks/roles.py")], {
    stdin: new Blob([JSON.stringify(cases)]),
    stdout: "pipe",
    stderr: "pipe",
  });
  const [out, err, code] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);

  if (code !== 0) throw new Error(err);
  return JSON.parse(out);
}

const keeps = { role_files: true };

describe("the gateway's role hook", () => {
  test("restores a role's file before the role's first request on an instance and not again while it is on it", async () => {
    const [first, again, upper] = await hook([
      { role: "chat", info: keeps },
      { role: "chat", info: keeps },
      { role: "chat", header: "X-BMO-Role", info: keeps },
    ]);

    expect(first?.asked).toEqual([{ path: "/slots/0?action=restore", file: "role-chat.bin", key: "Bearer node-key" }]);
    expect(first?.answer).toBeNull();
    expect(again?.asked).toEqual([]);
    expect(upper?.asked).toEqual([]);
  });

  test("takes the instance off its role on a request naming none, so the role's next request restores again", async () => {
    const [, none, back] = await hook([{ role: "chat", info: keeps }, { info: keeps }, { role: "chat", info: keeps }]);

    expect(none?.asked).toEqual([]);
    expect(back?.asked.map((one) => one.file)).toEqual(["role-chat.bin"]);
  });

  test("lets a request whose restore was refused go as it stands, and does not ask that role of the instance again", async () => {
    const [refused, again, other] = await hook([
      { role: "gate", info: keeps },
      { role: "gate", info: keeps },
      { role: "chat", info: keeps },
    ]);

    expect(refused?.asked.map((one) => one.file)).toEqual(["role-gate.bin"]);
    expect(refused?.answer).toBeNull();
    expect(again?.asked).toEqual([]);
    expect(other?.asked.map((one) => one.file)).toEqual(["role-chat.bin"]);
  });

  test("asks nothing of an instance that keeps no role files, nor for a name that is no role", async () => {
    const told = await hook([
      { role: "chat", info: null },
      { role: "chat", info: { role_files: false } },
      { role: "../chat", info: keeps },
      { role: "chat", info: keeps, path: "/v1/chat/completions" },
    ]);

    expect(told.slice(0, 3).map((one) => one.asked)).toEqual([[], [], []]);
    expect(told[3]?.asked.map((one) => one.path)).toEqual(["/slots/0?action=restore"]);
  });
});
