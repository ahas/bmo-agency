import { afterAll, beforeAll, describe, expect, test } from "bun:test";

import { emptyDatabase } from "@agency/test-support/store";

import { frontAlone, frontBinary } from "./control-contract-base";

import type { ControlBase } from "./control-contract-base";

// The pinned workers' programs stood again on a caller's asking (docs/agent-control.md, 「A worker
// standing with nothing to do」; docs/api.md, 「Workers」). The route answers at once and leaves the pass
// running; the test's store keeps no worker, so the pass finds none to stand, and the pass itself is held
// by the Rust standing pass's own tests.

const token = "secret";
// The binary is built as the file loads, so no hook spends its own time on a build.
const binary = frontBinary();

// Made at the file's top, as a scratch store must be.
const store = await emptyDatabase();

let front: ControlBase | null = null;

beforeAll(async () => {
  front = await frontAlone(binary, token, { BMO_ARCHIVE_URL: store });
});

afterAll(async () => {
  await front?.close();
});

describe("POST /workers/stand", () => {
  test("answers at once under the token, and refuses without it", async () => {
    const asked = (authorization: string) =>
      fetch(`http://127.0.0.1:${front!.port}/workers/stand`, {
        method: "POST",
        headers: { authorization, "content-type": "application/json" },
        body: JSON.stringify({ lockWaitSeconds: 60 }),
      });
    const stood = await asked(`Bearer ${token}`);

    expect([stood.status, await stood.json()]).toEqual([200, { ok: true }]);
    expect((await asked("Bearer wrong")).status).toBe(401);
  });
});
