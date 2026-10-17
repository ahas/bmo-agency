import { join } from "node:path";

// The tool host a launch asks for its run's token, stood in on a socket in `dir` and named to the code
// under test until it is stopped: every record is issued a token of its own, and every revocation is
// taken.
export function toolHostStandIn(dir: string): { issued: Record<string, unknown>[]; stop: () => void } {
  const previous = process.env.BMO_MCP_HOST_SOCKET;
  const socket = join(dir, "mcp-host.sock");
  const issued: Record<string, unknown>[] = [];
  const server = Bun.serve({
    unix: socket,
    async fetch(request) {
      const path = new URL(request.url).pathname;

      if (request.method === "POST" && path === "/sessions") {
        issued.push((await request.json()) as Record<string, unknown>);
        return Response.json({ token: `token-${issued.length}` });
      }

      return new Response(null, { status: request.method === "DELETE" ? 204 : 404 });
    },
  });

  process.env.BMO_MCP_HOST_SOCKET = socket;

  return {
    issued,
    stop: () => {
      server.stop(true);
      if (previous === undefined) delete process.env.BMO_MCP_HOST_SOCKET; else process.env.BMO_MCP_HOST_SOCKET = previous;
    },
  };
}

// Run as a process of its own, for a test whose code under test runs while the test itself waits on
// it: the stand-in serves on the socket in the directory named until it is killed.
if (import.meta.main) {
  const dir = process.argv[2];

  if (dir === undefined) throw new Error("usage: bun testbed/testkit/tool-host-stand-in.ts <dir>");

  toolHostStandIn(dir);
}
