// A binary of agency's own workspace as a TypeScript test starts it: built once per test process by
// Cargo in debug, into whichever target directory the run names and read from the path Cargo reports,
// on the account's own toolchain and Cargo home, which the test home does not move. Cargo is the one
// that knows the workspace's crates, so nothing here keeps a list of them.

import { join } from "node:path";

import { accountHome } from "@agency/support/roots";

// The account's own home, where its toolchain and Cargo home stand whatever HOME a test is given.
const ownHome = accountHome() ?? "/";

// The binaries this process has built already, so a test file asking twice builds once.
const built = new Map<string, string>();

/**
 * The path of `binary` of the workspace package `crate`, built now where this process has not built it
 * yet. A build that fails throws with what Cargo said.
 */
export function crateBinary(root: string, binary: string, crate: string = binary): string {
  const key = `${root}\n${crate}\n${binary}`;
  const known = built.get(key);

  if (known !== undefined) {
    return known;
  }

  const done = Bun.spawnSync([process.env.CARGO ?? "cargo", "build", "--quiet", "--message-format=json-render-diagnostics", "-p", crate, "--bin", binary], {
    cwd: root,
    env: {
      ...process.env,
      CARGO_HOME: process.env.CARGO_HOME ?? join(ownHome, ".cargo"),
      RUSTUP_HOME: process.env.RUSTUP_HOME ?? join(ownHome, ".rustup"),
    },
    stdout: "pipe",
    stderr: "pipe",
  });

  if (done.exitCode !== 0) {
    throw new Error(`cargo build -p ${crate} --bin ${binary} in ${root}:\n${done.stderr.toString()}`);
  }

  // Cargo says where it put the binary, one JSON message a line, wherever its target directory is.
  const path = done.stdout
    .toString()
    .split("\n")
    .filter((line) => line.startsWith("{"))
    .map((line) => JSON.parse(line) as { reason?: string; target?: { name?: string; kind?: string[] }; executable?: string | null })
    .find((message) => message.reason === "compiler-artifact" && message.target?.name === binary && message.target.kind?.includes("bin"))?.executable;

  if (path === undefined || path === null) {
    throw new Error(`cargo build -p ${crate} --bin ${binary} in ${root} named no binary it built`);
  }

  built.set(key, path);

  return path;
}
