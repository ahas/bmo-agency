import { existsSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { afterAll, describe, expect, test } from "bun:test";

import { scratchDir } from "@agency/test-support/scratch";

import { promptCacheTtlMs, promptCacheTtlSeconds, promptCacheTtlsSeconds, publishCacheTtl, sirenConfigPath, ttlPublish } from "@agency/agent/cache-ttl";

// How long a prompt cache entry lives is one fact, and two programs on this machine judge by it:
// the bot, deciding when a session is still worth keeping warm, and cache-siren, deciding whether
// a request that lost its prefix lost it to the clock or to the prefix. It used to be written
// down in both, and a value written twice goes out of step the day the API moves — quietly, since
// the watch would go on answering with the old number and nothing would say so.
//
// So it is written here and carried to the watch's own config, which the watch reads as it always
// has. What is held below is that the carrying really derives from the one number, that it leaves
// the rest of another program's file alone, and — on a machine that has the watch — that the two
// actually agree right now.

const made: string[] = [];

afterAll(() => {
  for (const dir of made) {
    rmSync(dir, { recursive: true, force: true });
  }
});

function scratch(): string {
  const dir = scratchDir("bmo-cache-ttl-");

  made.push(dir);

  return join(dir, "config.json");
}

describe("what the watch is told the cache's life is", () => {
  test("is the bot's own number and never a second copy of it", () => {
    expect(promptCacheTtlSeconds()).toBe(promptCacheTtlMs / 1000);
    expect(ttlPublish({}).next?.ttl_s).toBe(promptCacheTtlMs / 1000);
    // A config that says something else is corrected to the number above, whatever that number
    // becomes: this reads it rather than repeating it, so the two cannot be moved apart.
    expect(ttlPublish({ ttl_s: 1 }).next?.ttl_s).toBe(promptCacheTtlMs / 1000);
    expect(ttlPublish({}).next?.ttl_by_mode_model_s).toEqual(promptCacheTtlsSeconds());
  });

  test("nothing at all where it already says it", () => {
    const already = ttlPublish({
      port: 9022,
      ttl_s: promptCacheTtlSeconds(),
      ttl_by_mode_model_s: promptCacheTtlsSeconds(),
    });

    expect(already.next).toBeNull();
    expect(already.was).toBe(promptCacheTtlSeconds());
  });

  // A config written before the watch had the key falls back to the watch's own default, which is
  // the second copy this closes. It reads as no value rather than as agreement.
  test("a config with no such key is one that says nothing, not one that agrees", () => {
    expect(ttlPublish({ port: 9022 }).was).toBeNull();
    expect(ttlPublish({ ttl_s: "3600" }).was).toBeNull();
    expect(ttlPublish({ port: 9022 }).next).not.toBeNull();
  });

  // The file belongs to another program. One number in it is this side's business and the rest
  // of it is not.
  test("every other setting in that file is carried over untouched", () => {
    const before = { port: 9022, voice: false, voice_cmd: [], quota_url: "http://x", ttl_s: 1 };
    const after = ttlPublish(before).next ?? {};

    expect(after).toEqual({
      ...before,
      ttl_s: promptCacheTtlSeconds(),
      ttl_by_mode_model_s: promptCacheTtlsSeconds(),
    });
  });
});

describe("telling the watch", () => {
  test("writes the file and restarts the watch, since its config is read at its own start", async () => {
    const path = scratch();
    let restarts = 0;

    await Bun.write(path, JSON.stringify({ port: 9022, ttl_s: 60 }));

    const outcome = await publishCacheTtl({
      path,
      restart: async () => {
        restarts += 1;

        return true;
      },
    });

    expect(outcome).toBe("written");
    expect(restarts).toBe(1);
    expect(JSON.parse(readFileSync(path, "utf8")).ttl_s).toBe(promptCacheTtlSeconds());
    expect(JSON.parse(readFileSync(path, "utf8")).port).toBe(9022);
  });

  test("and does nothing the second time, so a start is not a restart of the watch", async () => {
    const path = scratch();
    let restarts = 0;
    const restart = async (): Promise<boolean> => {
      restarts += 1;

      return true;
    };

    await Bun.write(path, JSON.stringify({ port: 9022 }));
    expect(await publishCacheTtl({ path, restart })).toBe("written");
    expect(await publishCacheTtl({ path, restart })).toBe("agreed");
    expect(await publishCacheTtl({ path, restart })).toBe("agreed");
    expect(restarts).toBe(1);
  });

  test("a machine with no watch on it is left alone", async () => {
    const path = scratch();
    let restarts = 0;

    expect(
      await publishCacheTtl({
        path,
        restart: async () => {
          restarts += 1;

          return true;
        },
      }),
    ).toBe("absent");
    expect(restarts).toBe(0);
    expect(existsSync(path)).toBe(false);
  });

  test("a file that is not JSON is said so rather than overwritten", async () => {
    const path = scratch();

    await Bun.write(path, "not json at all");

    expect(await publishCacheTtl({ path, restart: async () => true })).toBe("failed");
    expect(readFileSync(path, "utf8")).toBe("not json at all");
  });
});

// The check that catches a real divergence rather than a hypothetical one. It reads the watch's
// own config as it stands on this machine and holds it against the bot's number. A machine with
// no watch installed has nothing to disagree with and says nothing.
describe("the watch on this machine", () => {
  const path = sirenConfigPath();

  test("judges a cold request by the same cache life the bot keeps sessions warm for", () => {
    if (!existsSync(path)) {
      expect(existsSync(path)).toBe(false);

      return;
    }

    const held = JSON.parse(readFileSync(path, "utf8")) as { ttl_s?: unknown };

    expect(held.ttl_s).toBe(promptCacheTtlSeconds());
  });
});
