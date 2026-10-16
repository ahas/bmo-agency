// The archive's `agents`, `legacy_chat_agents`, `bmo_rooms`, `bmo_session_rooms` and
// `bmo_room_contexts` tables, read and written here and nowhere else in the archive
// (docs/archive-schema.md, 「The query stores」).

import { archiveSeeds } from "./archive-seeding";
import { chatAgentConversation, companionConversation, conversationOf, defaultAnswerer, isCompanion } from "./conversational";
import type { Conversation, ConversationColumns } from "./conversational";
import type { AgentProviderName as AgentMode } from "./provider-names";
import type { WorkerSessionStore } from "./worker-session-store";
import type { ArchiveSql } from "./archive-sql";

// The mark a worker wears on a screen. Every worker is the same agent pointed at a different
// checkout, so they all wear it and a screen never has to know the names of any of them.
const WORKER_ICON = "i-material-symbols-construction";

export interface ChatSession {
  chatId: number;
  claudeSessionId: string;
  // When the session last replied, in seconds.
  lastReplyAt: number;
  // The Claude account the session was opened under. A session opened under one login cannot be
  // resumed under another, so a run on a different account is never offered this one. Null on a
  // record written before there was more than one login to open it under.
  account: string | null;
}

// What kind of agent a row is, as the deployment names its kinds (`record-names.ts` for agency's).
export type AgentKind = string;

// One agent the bot runs and what is known of its state. The definition of an agent — its launch
// line, its documents — lives with the agent's code; this is only the list and the record.
export interface AgentRecord {
  name: string;
  kind: AgentKind;
  // What the agent is for, which decides the function bundles its turns are given
  // (docs/agent-roles.md); null on a row no settling or seed has named one for.
  role: string | null;
  // The road that most recently answered. What an agent is configured to run on is not the row's:
  // it is the tier store's answer for the agent's work key, and the model a
  // held session runs on is that session's own record.
  lastMode: AgentMode | null;
  lastModel: string | null;
  lastEffort: string | null;
  // What the owner calls this agent, and the other names the owner calls it by. Both are the archive's
  // to answer rather than any screen's or any document's: the catalogue is one list, here.
  label: string | null;
  aliases: string | null;
  // The name herdr shows it under, where the owner wants it shown there under another than its
  // label; null where herdr shows the label.
  herdrLabel: string | null;
  // The mark a screen puts beside it.
  icon: string | null;
  // Where a worker's jobs run. Null for every other kind, which runs where its line says.
  cwd: string | null;
  // The node a worker's checkout stands on, its directory being a path on that node; null for this
  // machine, and on every row that is no worker (docs/work-nodes.md).
  node: string | null;
  // What the owner has a worker's job prompts open with, and the one line of shell its standing
  // dev server runs. Null for every other kind, and for a worker the owner has written nothing for.
  preamble: string | null;
  devCommand: string | null;
  // Whether the bot writes this row on every start. A built-in agent is part of the machine and
  // cannot be removed; one the owner added is the owner's to remove.
  builtin: boolean;
  chatId: number | null;
  // The Claude account the session below was opened under, and null where the row holds none.
  account: string | null;
  sessionId: string | null;
  lastRunAt: number | null;
  lastKeepaliveAt: number | null;
  // What the session was carrying when it last answered, in tokens. It is what resuming that
  // session would cost once its prefix has been invalidated, and so one side of the choice
  // between resuming it and opening a new one. Null until a run has reported it.
  contextTokens: number | null;
  // When a document that makes up this chat's own system prompt was last written. A session
  // that answered before this has lost its cached prefix whether it has noticed or not, so its
  // next turn writes the whole context again — which is when the bot decides whether that
  // context is worth writing. Only the chats a document actually concerns are marked: a room's
  // own persona file moves that room's prompt and nobody else's.
  promptChangedAt: number | null;
  // Whether the owner has stopped this agent's keepalive. A stopped agent is passed over by the
  // warming round and its record is left where it stands, so nothing pretends its prefix is warm.
  keepaliveStopped: boolean;
  // When the owner turned a worker's keepalive off until its next job, in seconds, or null. It is
  // not a switch: the next job of that worker takes it away, and the warming comes back with it.
  keepalivePausedAt: number | null;
  // Whether a worker whose run came back on a lesser model has that session dropped. On unless the
  // owner turned it off for that worker, who then goes on in the session on the model it fell to.
  downgradeHeal: boolean;
  // A base worker's two support settings — how many support workers it may have and how many of
  // them are kept warm — and, on a support worker's own row, whose it is and its number
  // (`docs/worker-slots.md`). Zero, zero, null and null on every row they do not apply to.
  supportCount: number;
  supportKeepalive: number;
  supportOf: string | null;
  supportSlot: number | null;
  // The one provider a worker is pinned to, whose tier mapping for the worker's key its jobs and the
  // words said to it run on; null where it follows the worker key's road like every other worker.
  // On a companion, the provider its every turn goes out on (docs/companions.md). Null on every
  // other row.
  pinnedProvider: AgentMode | null;
  // The one model of the pinned provider the worker runs on, where the provider takes one named — a
  // registered model; null where the pin is the provider alone.
  pinnedModel: string | null;
  // The agent's own thinking and effort over its tier (docs/model-tiers.md, 「An agent's own thinking
  // and effort」): whether it thinks, and the level of the one scale it runs at, each null where the
  // tier and the line decide.
  thinking: boolean | null;
  effort: string | null;
  // The worker a companion's TODO items go to; null on every row that is no companion.
  itemWorker: string | null;
  // A designated agent's own values (docs/agent-ownership.md): the brain folder its long-term memory
  // lives in, relative to the brain root, empty for the root itself; the voice its words are spoken
  // in, or none; how long a room it answered keeps it listening without a call, or none; and the
  // words that call it beside its label. Null on every other row.
  brainFolder: string | null;
  voice: string | null;
  windowSeconds: number | null;
  wakeWords: string[];
  // How the one runner carries it, where it is a conversational agent (docs/agents.md, 「One runner」);
  // null on every other row.
  conversation: Conversation | null;
  runs: number;
}

interface AgentRow extends ConversationColumns {
  name: string;
  kind: AgentKind;
  // Absent on a file the settling has not given the column yet.
  role?: string | null;
  last_mode: AgentMode | null;
  last_model: string | null;
  last_effort: string | null;
  label: string | null;
  aliases: string | null;
  herdr_label: string | null;
  icon: string | null;
  cwd: string | null;
  // Absent on a file the settling has not given the column yet.
  node?: string | null;
  preamble: string | null;
  dev_command: string | null;
  builtin: number;
  chat_id: number | null;
  account: string | null;
  session_id: string | null;
  last_run_at: number | null;
  last_keepalive_at: number | null;
  context_tokens: number | null;
  prompt_changed_at: number | null;
  keepalive_stopped: number;
  keepalive_paused_at: number | null;
  downgrade_heal: number;
  support_count: number;
  support_keepalive: number;
  support_of: string | null;
  support_slot: number | null;
  pinned_provider: AgentMode | null;
  pinned_model: string | null;
  thinking: number | null;
  effort: string | null;
  // Absent on a file the settling has not given the column yet.
  item_worker?: string | null;
  brain_folder?: string | null;
  voice?: string | null;
  window_seconds?: number | null;
  wake_words?: string | null;
  runs: number;
}

// The role a row the store makes holds, as the deployment names it for the row's kind; null where it
// names none.
function roleOfKind(kind: AgentKind): string | null {
  return archiveSeeds().roleOfKind[kind] ?? null;
}

// Names kept as one string, the way a row carries them, as the list they are.
function wordList(text: string | null): string[] {
  return (text ?? "").split(",").map((word) => word.trim()).filter((word) => word !== "");
}

// A designated agent's own values an owner or a seed sets (docs/agent-ownership.md); a field left out
// stays as it stands, and null clears it.
export interface AgentOwnValues {
  brainFolder?: string | null;
  voice?: string | null;
  windowSeconds?: number | null;
  wakeWords?: readonly string[] | null;
}

export class AgentStore {
  constructor(
    private readonly sql: ArchiveSql,
    private readonly workerSessions: WorkerSessionStore,
  ) {}

  private agentOf(row: AgentRow): AgentRecord {
    return {
      name: row.name,
      kind: row.kind,
      role: row.role ?? null,
      lastMode: row.last_mode,
      lastModel: row.last_model,
      lastEffort: row.last_effort,
      label: row.label,
      aliases: row.aliases,
      herdrLabel: row.herdr_label ?? null,
      icon: row.icon,
      cwd: row.cwd,
      node: row.node ?? null,
      preamble: row.preamble,
      devCommand: row.dev_command,
      builtin: row.builtin === 1,
      chatId: row.chat_id,
      account: row.account ?? null,
      sessionId: row.session_id,
      lastRunAt: row.last_run_at,
      lastKeepaliveAt: row.last_keepalive_at,
      contextTokens: row.context_tokens,
      promptChangedAt: row.prompt_changed_at,
      keepaliveStopped: row.keepalive_stopped === 1,
      keepalivePausedAt: row.keepalive_paused_at ?? null,
      downgradeHeal: row.downgrade_heal !== 0,
      supportCount: row.support_count ?? 0,
      supportKeepalive: row.support_keepalive ?? 0,
      supportOf: row.support_of ?? null,
      supportSlot: row.support_slot ?? null,
      pinnedProvider: row.pinned_provider ?? null,
      pinnedModel: row.pinned_provider === null ? null : row.pinned_model ?? null,
      thinking: row.thinking == null ? null : row.thinking !== 0,
      effort: row.effort ?? null,
      itemWorker: row.item_worker ?? null,
      brainFolder: row.brain_folder ?? null,
      voice: row.voice ?? null,
      windowSeconds: row.window_seconds ?? null,
      wakeWords: wordList(row.wake_words ?? null),
      conversation: conversationOf(row),
      runs: row.runs,
    };
  }

  // Lists an agent, keeping whatever state it already has. A fixed agent is registered on every
  // start; an agent that has never run counts as having run now, so its first keepalive is one
  // idle stretch away rather than immediate.
  async registerAgent(agent: {
    name: string;
    kind: AgentKind;
    chatId?: number;
    label?: string;
    aliases?: string;
    icon?: string;
    cwd?: string;
    role?: string;
  }): Promise<void> {
    await this.sql.run(`
        INSERT INTO agents (name, kind, chat_id, last_run_at, label, aliases, icon, cwd, builtin, role)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, 1, ?)
        ON CONFLICT(name) DO UPDATE SET
          kind = excluded.kind,
          builtin = 1,
          -- What the start writes is the machine's own account of the agent, and what a row
          -- already carries is the owner's: a name the owner gave it, or a directory the owner pointed it at,
          -- stands rather than being written over every time the bot comes up.
          label = COALESCE(agents.label, excluded.label),
          aliases = COALESCE(agents.aliases, excluded.aliases),
          icon = COALESCE(agents.icon, excluded.icon),
          cwd = COALESCE(agents.cwd, excluded.cwd),
          role = COALESCE(agents.role, excluded.role)
      `, agent.name, agent.kind, agent.chatId ?? null, Math.floor(Date.now() / 1000), agent.label ?? null, agent.aliases ?? null, agent.icon ?? null, agent.cwd ?? null, agent.role ?? roleOfKind(agent.kind));
  }

  // A worker the owner added: a row of the same catalogue, marked as the owner's rather than the
  // machine's, so the next start neither writes over it nor claims it.
  async addWorker(worker: {
    name: string;
    label: string;
    cwd: string;
    preamble?: string | null;
    devCommand?: string | null;
  }): Promise<void> {
    await this.sql.run(`
        INSERT INTO agents (name, kind, label, cwd, icon, builtin, last_run_at, preamble, dev_command, role)
        VALUES (?, 'worker', ?, ?, ?, 0, ?, ?, ?, ?)
      `, worker.name, worker.label, worker.cwd, WORKER_ICON, Math.floor(Date.now() / 1000), worker.preamble ?? null, worker.devCommand ?? null, roleOfKind("worker"));
  }

  // What the owner changed about a worker. Only what was named moves; the rest of the row — its
  // runs, its last run, the session it holds — is the record and is not the owner's to write.
  async updateWorker(
    name: string,
    changes: {
      label?: string;
      cwd?: string;
      preamble?: string | null;
      devCommand?: string | null;
      downgradeHeal?: boolean;
      keepaliveStopped?: boolean;
      supportCount?: number;
      supportKeepalive?: number;
      pinnedProvider?: AgentMode | null;
      // Written with the provider: a pin to a provider alone clears it.
      pinnedModel?: string | null;
      // Null gives herdr the label back.
      herdrLabel?: string | null;
      // Null stands the worker on this machine.
      node?: string | null;
    },
  ): Promise<boolean> {
    const sets: string[] = [];
    const args: (string | number | null)[] = [];

    if (changes.label !== undefined) {
      sets.push("label = ?");
      args.push(changes.label);
    }

    if (changes.cwd !== undefined) {
      sets.push("cwd = ?");
      args.push(changes.cwd);
    }

    if (changes.preamble !== undefined) {
      sets.push("preamble = ?");
      args.push(changes.preamble);
    }

    if (changes.devCommand !== undefined) {
      sets.push("dev_command = ?");
      args.push(changes.devCommand);
    }

    if (changes.downgradeHeal !== undefined) {
      sets.push("downgrade_heal = ?");
      args.push(changes.downgradeHeal ? 1 : 0);
    }

    if (changes.keepaliveStopped !== undefined) {
      sets.push("keepalive_stopped = ?");
      args.push(changes.keepaliveStopped ? 1 : 0);
    }

    if (changes.supportCount !== undefined) {
      sets.push("support_count = ?");
      args.push(changes.supportCount);
    }

    if (changes.supportKeepalive !== undefined) {
      sets.push("support_keepalive = ?");
      args.push(changes.supportKeepalive);
    }

    if (changes.pinnedProvider !== undefined) {
      sets.push("pinned_provider = ?", "pinned_model = ?");
      args.push(changes.pinnedProvider, changes.pinnedProvider === null ? null : changes.pinnedModel ?? null);
    }

    if (changes.herdrLabel !== undefined) {
      sets.push("herdr_label = ?");
      args.push(changes.herdrLabel);
    }

    if (changes.node !== undefined) {
      sets.push("node = ?");
      args.push(changes.node);
    }

    if (sets.length === 0) {
      return false;
    }

    return (
      (await this.sql.run(`UPDATE agents SET ${sets.join(", ")} WHERE name = ? AND kind = 'worker'`, ...args, name)).changes > 0
    );
  }

  // An agent's own thinking and effort, each null to leave it to the tier and the line. Whether a
  // level is one of the scale is the caller's to judge; this only writes the row. False where no
  // agent has the name.
  async setReasoning(name: string, reasoning: { thinking: boolean | null; effort: string | null }): Promise<boolean> {
    return (await this.sql.run("UPDATE agents SET thinking = ?, effort = ? WHERE name = ?", reasoning.thinking === null ? null : reasoning.thinking ? 1 : 0, reasoning.effort, name)).changes > 0;
  }

  // The workers pinned to one model of a provider, by name.
  async workersPinnedTo(provider: AgentMode, model: string): Promise<string[]> {
    return (await this.sql.all(`SELECT name FROM agents WHERE kind = 'worker' AND ${await this.awake()} AND pinned_provider = ? AND pinned_model = ? ORDER BY name`, provider, model) as { name: string }[]).map((row) => row.name);
  }

  // A companion pinned to one model of one provider (docs/companions.md, 「Its settings」).
  async pinCompanion(name: string, provider: AgentMode, model: string): Promise<boolean> {
    if (!isCompanion(await this.conversationOf(name))) return false;

    return (await this.sql.run("UPDATE agents SET pinned_provider = ?, pinned_model = ? WHERE name = ?", provider, model, name)).changes > 0;
  }

  // The companions pinned to one model of one provider, by name.
  async companionsPinnedTo(provider: AgentMode, model: string): Promise<string[]> {
    const rows = await this.sql.all("SELECT * FROM agents WHERE pinned_provider = ? AND pinned_model = ? ORDER BY name", provider, model) as AgentRow[];

    return rows.filter((row) => isCompanion(conversationOf(row))).map((row) => row.name);
  }

  // Removing a worker the owner added. A built-in is refused: the bot writes it on every start,
  // so deleting it would only make it come back with its record gone.
  async removeWorker(name: string): Promise<boolean> {
    return await this.sql.wrote(async () => {
      const gone =
        (await this.sql.run("DELETE FROM agents WHERE name = ? AND kind = 'worker' AND builtin = 0", name)).changes > 0;

      if (gone) {
        await this.workerSessions.deleteWorkerSessions(name);
      }

      return gone;
    });
  }

  // A support worker of a base, written as the base's setting settles it (`docs/worker-slots.md`).
  // It is the machine's row rather than the owner's: what it is called and where it works follow
  // from the base, and it goes when the count comes down.
  async addSupportWorker(worker: { name: string; label: string; cwd: string; base: string; slot: number }): Promise<void> {
    await this.sql.run(`
        INSERT INTO agents (name, kind, label, cwd, icon, builtin, last_run_at, support_of, support_slot, role)
        VALUES (?, 'worker', ?, ?, ?, 0, ?, ?, ?, ?)
      `, worker.name, worker.label, worker.cwd, WORKER_ICON, Math.floor(Date.now() / 1000), worker.base, worker.slot, roleOfKind("worker"));
  }

  // A base's checkout on a node, a support worker of its own placed there and numbered by no slot
  // (docs/work-nodes.md, 「A checkout on a node」). Like a numbered one it is the base's row: added and
  // removed through the base's settings alone.
  async addNodeCheckout(worker: { name: string; label: string; cwd: string; base: string; node: string }): Promise<void> {
    await this.sql.run(`
        INSERT INTO agents (name, kind, label, cwd, icon, builtin, last_run_at, support_of, node, role)
        VALUES (?, 'worker', ?, ?, ?, 0, ?, ?, ?, ?)
      `, worker.name, worker.label, worker.cwd, WORKER_ICON, Math.floor(Date.now() / 1000), worker.base, worker.node, roleOfKind("worker"));
  }

  // A support worker taken away with its sessions, where the base's count no longer reaches it.
  async removeSupportWorker(name: string): Promise<boolean> {
    return await this.sql.wrote(async () => {
      const gone = (await this.sql.run("DELETE FROM agents WHERE name = ? AND kind = 'worker' AND support_of IS NOT NULL", name)).changes > 0;

      if (gone) {
        await this.workerSessions.deleteWorkerSessions(name);
      }

      return gone;
    });
  }

  // Every worker, the built-in first and the rest by name, each base followed by its support workers
  // in their order, which is the order a list of them reads in. A resting worker is no worker of
  // this list (`docs/jobs.md`, 「There can be more than one job worker」).
  async listWorkers(): Promise<AgentRecord[]> {
    const rows = await this.sql.all(`
        SELECT a.* FROM agents a WHERE a.kind = 'worker' AND ${await this.awake("a.")}
        ORDER BY COALESCE((SELECT b.builtin FROM agents b WHERE b.name = a.support_of), a.builtin) DESC,
          COALESCE(a.support_of, a.name), COALESCE(a.support_slot, 1)
      `) as AgentRow[];

    return rows.map((row) => this.agentOf(row));
  }

  // Every agent but a resting worker, which keeps its row for its record alone.
  async listAgents(): Promise<AgentRecord[]> {
    const rows = await this.sql.all(`SELECT * FROM agents WHERE ${await this.awake()} ORDER BY kind, name`) as AgentRow[];

    return rows.map((row) => this.agentOf(row));
  }

  // Whether the file holds the workers' rest. The close of a job and the backlog deployment open an
  // archive the settling has not reached yet and read the workers on the way; a file without the
  // column has no resting worker, and is answered as such rather than refused.
  private restSchema: boolean | null = null;

  private async holdsRest(): Promise<boolean> {
    if (this.restSchema === null) {
      this.restSchema = (await this.sql.columnNames("agents")).includes("resting_since");
    }

    return this.restSchema;
  }

  // The chat agent: the conversational agent whose row says it answers every room that names nobody, and
  // the deployment's own where no row says so (docs/agents.md, 「One runner」).
  async defaultAnswerer(): Promise<string> {
    return await defaultAnswerer(this.sql);
  }

  // How the one runner carries an agent, or null where it has no row or its row is no conversational
  // agent's.
  async conversationOf(name: string): Promise<Conversation | null> {
    const row = await this.sql.get("SELECT * FROM agents WHERE name = ?", name) as AgentRow | null;

    return row === null ? null : conversationOf(row);
  }

  // A companion as its deployment declares it, written where it is missing: a conversational agent of
  // the kind `chat` keeping a session in each room and taking its settings' pieces as its turn. What a
  // row already standing holds — its kind, label, mark, pin, worker, prompt folder and settings — is the
  // owner's and stands (docs/companions.md, 「Where a companion and its rooms come from」).
  async standCompanion(companion: { name: string; label: string; icon: string; provider: string; model: string; itemWorker: string; thinking?: boolean; role?: string; prompts?: string }): Promise<void> {
    return await this.sql.wrote(async () => {
      // A pin is the provider and the model together, so a row pinned already keeps both; whether it
      // thinks is the seed's only for a row made now. A row an older writer left without the settings
      // is given them all together, so its scope, shape and answering never stand apart.
      await this.sql.run(`
          INSERT INTO agents (name, kind, session_scope, turn_shape, default_answerer, label, icon, pinned_provider, pinned_model, item_worker, thinking, builtin, last_run_at, role, prompts)
          VALUES (?, 'chat', ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?)
          ON CONFLICT(name) DO UPDATE SET
            builtin = 1,
            turn_shape = CASE WHEN agents.session_scope IS NULL THEN excluded.turn_shape ELSE agents.turn_shape END,
            default_answerer = CASE WHEN agents.session_scope IS NULL THEN excluded.default_answerer ELSE agents.default_answerer END,
            session_scope = COALESCE(agents.session_scope, excluded.session_scope),
            label = COALESCE(agents.label, excluded.label),
            icon = COALESCE(agents.icon, excluded.icon),
            pinned_model = CASE WHEN agents.pinned_provider IS NULL THEN excluded.pinned_model ELSE agents.pinned_model END,
            pinned_provider = COALESCE(agents.pinned_provider, excluded.pinned_provider),
            item_worker = COALESCE(agents.item_worker, excluded.item_worker),
            role = COALESCE(agents.role, excluded.role),
            prompts = COALESCE(agents.prompts, excluded.prompts)
        `, companion.name, companionConversation.scope, companionConversation.shape, companionConversation.defaultAnswerer ? 1 : 0, companion.label, companion.icon, companion.provider, companion.model, companion.itemWorker, companion.thinking === undefined ? null : companion.thinking ? 1 : 0, Math.floor(Date.now() / 1000), companion.role ?? roleOfKind("chat"), companion.prompts ?? null);
      // Its long-term memory is a folder of its own and its aliases call it, where nothing has been set
      // (docs/agent-ownership.md); it has no voice and no window until the owner gives it one.
      await this.sql.run("UPDATE agents SET brain_folder = COALESCE(brain_folder, 'agents/' || name), wake_words = COALESCE(wake_words, aliases) WHERE name = ?", companion.name);
    });
  }

  // The role an agent holds from now on (docs/agent-roles.md). Whether the role is one the deployment
  // declares is its caller's to judge; false where no such agent stands.
  async setRole(name: string, role: string): Promise<boolean> {
    return (await this.sql.run("UPDATE agents SET role = ? WHERE name = ?", role, name)).changes > 0;
  }

  // A designated agent's own values, each set where it is given (docs/agent-ownership.md).
  async setOwnValues(name: string, values: AgentOwnValues): Promise<boolean> {
    const sets: string[] = [];
    const params: (string | number | null)[] = [];

    if (values.brainFolder !== undefined) {
      sets.push("brain_folder = ?");
      params.push(values.brainFolder);
    }

    if (values.voice !== undefined) {
      sets.push("voice = ?");
      params.push(values.voice);
    }

    if (values.windowSeconds !== undefined) {
      sets.push("window_seconds = ?");
      params.push(values.windowSeconds);
    }

    if (values.wakeWords !== undefined) {
      sets.push("wake_words = ?");
      params.push(values.wakeWords === null ? null : values.wakeWords.join(", "));
    }

    if (sets.length === 0) return await this.getAgent(name) !== null;

    return this.sql.transaction(async () => (await this.sql.run(`UPDATE agents SET ${sets.join(", ")} WHERE name = ?`, ...params, name)).changes > 0);
  }

  // The condition a row of a worker at work meets, on the row the alias names: none resting.
  private async awake(alias = ""): Promise<string> {
    return await this.holdsRest() ? `${alias}resting_since IS NULL` : "1 = 1";
  }

  // Every name a row stands under, a resting worker's included: the names a new agent may not take.
  async takenNames(): Promise<string[]> {
    return (await this.sql.all("SELECT name FROM agents ORDER BY name") as { name: string }[]).map((row) => row.name);
  }

  // Whether a worker rests: its row stands, and nothing that stands a worker up takes it.
  async isResting(name: string): Promise<boolean> {
    if (!await this.holdsRest()) return false;

    return await this.sql.get("SELECT 1 FROM agents WHERE name = ? AND resting_since IS NOT NULL", name) !== null;
  }

  // The resting workers, by name, each with what it is called and since when it rests.
  async restingWorkers(): Promise<{ name: string; label: string | null; since: number }[]> {
    if (!await this.holdsRest()) return [];

    return await this.sql.all("SELECT name, label, resting_since AS since FROM agents WHERE kind = 'worker' AND resting_since IS NOT NULL ORDER BY name") as { name: string; label: string | null; since: number }[];
  }

  // Sets a worker the owner added to rest, or wakes it. It answers whether there was such a worker:
  // the built-in worker and a support worker never rest, and a worker already resting keeps the time
  // it began.
  async setWorkerResting(name: string, resting: boolean): Promise<boolean> {
    return await this.sql.wrote(async () => {
      if (!await this.holdsRest()) return false;

      return (
        (await this.sql.run(`UPDATE agents SET resting_since = ${resting ? "COALESCE(resting_since, ?)" : "NULL"} WHERE name = ? AND kind = 'worker' AND builtin = 0 AND support_of IS NULL`, ...(resting ? [Math.floor(Date.now() / 1000), name] : [name]))).changes > 0
      );
    });
  }

  // Whether the chat agent keeps one session across every room it answers (docs/unified-bmo.md).
  async isBmoUnified(): Promise<boolean> {
    return await this.oneSessionAgent() !== null;
  }

  async chatAgentName(chatId: number): Promise<string> {
    return await this.oneSessionAgent() ?? `chat:${chatId}`;
  }

  async canonicalAgent(name: string): Promise<string> {
    return /^chat:-?\d+$/.test(name) ? await this.oneSessionAgent() ?? name : name;
  }

  // The chat agent where its row keeps one session across every room it answers, or null where each
  // room's chat still keeps a row of its own.
  private async oneSessionAgent(): Promise<string | null> {
    const agent = await this.defaultAnswerer();

    return (await this.conversationOf(agent))?.scope === "agent" ? agent : null;
  }

  async getAgent(name: string): Promise<AgentRecord | null> {
    name = await this.canonicalAgent(name);
    const row = await this.sql.get("SELECT * FROM agents WHERE name = ?", name) as AgentRow | null;

    return row === null ? null : this.agentOf(row);
  }

  // Records that an agent ran just now.
  async noteRun(name: string): Promise<void> {
    name = await this.canonicalAgent(name);
    await this.sql.wrote(() => this.sql.run("UPDATE agents SET last_run_at = ?, runs = runs + 1 WHERE name = ?", Math.floor(Date.now() / 1000), name));
  }

  // What actually answered the turn, without changing the configured model or the resumable
  // Claude session. An unregistered one-off caller has no catalogue row to attach this to and is
  // simply left to its usage row.
  async noteAgentAnswer(name: string, mode: AgentMode, model: string | null, effort: string | null): Promise<void> {
    return await this.sql.wrote(async () => {
      name = await this.canonicalAgent(name);
      const changed = (await this.sql.run("UPDATE agents SET last_mode = ?, last_model = ?, last_effort = ? WHERE name = ?", mode, model, effort, name)).changes;

      // A room whose very first primary call failed can be answered by another harness before it
      // ever had a Claude session to make its catalogue row. The answer still makes it a real chat agent.
      const chat = /^chat:(-?\d+)$/.exec(name);

      if (changed === 0 && chat !== null) {
        await this.sql.run(`
            INSERT INTO agents (name, kind, chat_id, last_mode, last_model, last_effort, last_run_at)
            VALUES (?, 'chat', ?, ?, ?, ?, ?) ON CONFLICT DO NOTHING
          `, name, Number(chat[1]), mode, model, effort, Math.floor(Date.now() / 1000));
      }
    });
  }

  // Records a keepalive, which counts as a run for idleness.
  async noteKeepalive(name: string): Promise<void> {
    return await this.sql.wrote(async () => {
      name = await this.canonicalAgent(name);
      const now = Math.floor(Date.now() / 1000);

      await this.sql.run("UPDATE agents SET last_run_at = ?, last_keepalive_at = ?, runs = runs + 1 WHERE name = ?", now, now, name);
    });
  }

  // Stops one agent's keepalive, or starts it again. It answers whether there was a row to write
  // on: an agent this archive has never listed is a name nobody can switch.
  async setAgentKeepalive(name: string, stopped: boolean): Promise<boolean> {
    return await this.sql.wrote(async () => {
      name = await this.canonicalAgent(name);
      return (
        (await this.sql.run("UPDATE agents SET keepalive_stopped = ? WHERE name = ?", stopped ? 1 : 0, name)).changes > 0
      );
    });
  }

  // Turns a worker's keepalive off until its next job. It answers whether there was a worker of
  // that name to turn off: the pause is a worker's alone, since only a worker is woken by a job.
  async pauseWorkerKeepalive(name: string): Promise<boolean> {
    return (
      (await this.sql.run("UPDATE agents SET keepalive_paused_at = ? WHERE name = ? AND kind = 'worker'", Math.floor(Date.now() / 1000), name)).changes > 0
    );
  }

  // A job of the worker has started, which is the waking the pause was waiting for.
  async wakeWorkerKeepalive(name: string): Promise<void> {
    await this.sql.run("UPDATE agents SET keepalive_paused_at = NULL WHERE name = ?", name);
  }

  // Agents whose last run is older than the threshold, oldest first, a resting worker left out.
  async idleAgents(idleMs: number): Promise<AgentRecord[]> {
    const cutoff = Math.floor((Date.now() - idleMs) / 1000);
    const rows = await this.sql.all(`SELECT * FROM agents WHERE (last_run_at IS NULL OR last_run_at <= ?) AND ${await this.awake()} ORDER BY last_run_at`, cutoff) as AgentRow[];

    return rows.map((row) => this.agentOf(row));
  }

  async getSession(chatId: number): Promise<ChatSession | null> {
    const agent = await this.getAgent(`chat:${chatId}`);

    if (agent === null || agent.sessionId === null) {
      return null;
    }

    return {
      chatId,
      claudeSessionId: agent.sessionId,
      lastReplyAt: agent.lastRunAt ?? 0,
      account: agent.account,
    };
  }

  // Records the session a chat holds and that it replied just now.
  // The account is what the caller knows and the row is what remembers it: a caller that names
  // none has the row name none either, and a run that names an account is never handed a session
  // recorded under a different one.
  async setSession(
    session: Omit<ChatSession, "lastReplyAt" | "account"> & { account?: string | null },
    contextTokens: number | null = null,
  ): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run(`
          INSERT INTO agents (name, kind, chat_id, session_id, account, last_run_at, context_tokens, runs)
          VALUES (?, 'chat', ?, ?, ?, ?, ?, 1)
          ON CONFLICT(name) DO UPDATE SET
            session_id = excluded.session_id,
            account = excluded.account,
            last_run_at = excluded.last_run_at,
            context_tokens = COALESCE(excluded.context_tokens, agents.context_tokens),
            runs = agents.runs + 1
        `, await this.chatAgentName(session.chatId), await this.isBmoUnified() ? null : session.chatId, session.claudeSessionId, session.account ?? null, Math.floor(Date.now() / 1000), contextTokens);
    });
  }

  // The session a chat holds, forgotten. The agent's row stays with everything else it records;
  // only the session id goes, so the next message in that chat opens a fresh one.
  async clearSession(chatId: number): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run("UPDATE agents SET session_id = NULL, account = NULL, context_tokens = NULL WHERE name = ?", await this.chatAgentName(chatId));
    });
  }

  // A chat's session dropped in favour of a new one. The session id and the size that went
  // with it go, and the chat is marked as having run now: it is about to, and the mark is what
  // stops the same written document from being answered with a second replacement on the next
  // round before the new session has recorded anything of its own.
  async noteSessionReplaced(chatId: number, at: number = Math.floor(Date.now() / 1000)): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run("UPDATE agents SET session_id = NULL, context_tokens = NULL, last_run_at = ? WHERE name = ?", at, await this.chatAgentName(chatId));
    });
  }

  // What an agent that keeps no session was carrying at the end of its last run. A runner and an
  // editor open a fresh context every time they run, so this is not what the agent is holding
  // now — nothing is holding it — but the size the last run reached, which is what there is to
  // read about how much one of its runs carries. A run that reported no figure writes none:
  // silence leaves the last real reading standing rather than erasing it.
  async noteAgentContext(name: string, contextTokens: number | null): Promise<void> {
    return await this.sql.wrote(async () => {
      name = await this.canonicalAgent(name);
      if (contextTokens === null) {
        return;
      }

      await this.sql.run("UPDATE agents SET context_tokens = ? WHERE name = ?", contextTokens, name);
    });
  }

  // A chat's session folded: the size recorded for it was the size before the fold and says
  // nothing about what it now carries, so it goes. Unknown reads as never cut, which is the
  // safe way round.
  async noteSessionCompacted(chatId: number): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run("UPDATE agents SET context_tokens = NULL WHERE name = ?", await this.chatAgentName(chatId));
    });
  }

  // Records that a document making up this chat's own system prompt was written. Only the
  // chats a document concerns are marked, so a room's own persona file never costs another
  // room its memory.
  async notePromptChanged(chatId: number, at: number): Promise<void> {
    return await this.sql.wrote(async () => {
      await this.sql.run("UPDATE agents SET prompt_changed_at = ? WHERE name = ?", at, await this.chatAgentName(chatId));
    });
  }

  async sessionChatIds(): Promise<number[]> {
    const rows = await this.sql.all("SELECT * FROM agents WHERE chat_id IS NOT NULL AND session_id IS NOT NULL") as AgentRow[];

    return rows.filter((row) => conversationOf(row)?.defaultAnswerer === true).map((row) => row.chat_id!);
  }
}
