# AGENTS.md — Bazaar Plugin Marketplace

Bazaar is a Claude Code plugin marketplace hosted at `github.com/89jobrien/bazaar`. It serves
plugins via the Claude Code plugin system. This file is the agent context reference — install
steps, plugin inventory, skill triggers, and inter-plugin dependencies.

## Install Bazaar

Register the marketplace once per machine:

```bash
claude plugin marketplace add https://github.com/89jobrien/bazaar
```

Then install any plugin by name:

```bash
claude plugin install <name>@bazaar
```

## Plugin Index

| Plugin       | Purpose                                                      | Requires       |
| ------------ | ------------------------------------------------------------ | -------------- |
| sanctum      | 1Password auth + direnv chain tracing — runs at SessionStart | `op`, `direnv` |
| atelier      | Rust dev workflow — gates, review, CI, git safety, handoffs  | sanctum        |
| godmode      | TDD methodology + CLI task graph                             | godmode-cli    |
| valerie      | Todo/task management via doob CLI                            | doob           |
| orca-strait  | Parallel TDD sub-agent orchestrator for Rust workspaces      | gh, nextest    |
| cannibalizer | Absorb foreign repos into hexagonal Rust components          | cnbl binary    |

---

## sanctum

```bash
claude plugin install sanctum@bazaar
```

Runs a `SessionStart` hook automatically on every new Claude session:

1. Validates `op account list`
2. Traces `.envrc` `source_up` chain from CWD
3. Counts `op://` refs, flags literal URIs in environment
4. Returns a readiness summary to Claude

**Skill:** `/sanctum:op-resolver` — invoke on-demand mid-session for secrets debugging.

**Prerequisites:** `op` CLI on PATH. `direnv` optional (chain tracing degrades gracefully).

---

## atelier

```bash
claude plugin install atelier@bazaar
```

Requires sanctum for the session-start hook chain.

**Skills:**

| Skill              | Trigger phrases                                          |
| ------------------ | -------------------------------------------------------- |
| handon             | "start session", "what's outstanding", `/atelier:handon` |
| handoff            | "write handoff", "end of session"                        |
| handover           | "visualize the handoff", "show handoff diagrams"         |
| cargo-gate         | "run gates", "validate rust", "pre-commit check"         |
| sentinel-autofixer | "apply review fixes", "fix sentinel suggestions"         |
| hook-diagnostics   | "show hook status", "what hooks ran"                     |
| git-guard          | "safe to commit", "check merge strategy"                 |
| ci-assist          | "fix CI", "check cross-compile", "verify binary"         |
| project-pulse      | "end session", "capture state", "session summary"        |

**Agents:** sentinel, forge, herald, conductor, oxidizer (all thin wrappers; devkit must be
installed).

**Note:** `cargo-gate` runs `cargo xtask pre-commit` first — the xtask gate takes priority.

---

## godmode

```bash
# Install the CLI first
cargo install --path ~/dev/godmode/crates/godmode-cli --root ~/.local

# Install the plugin
claude plugin install godmode@bazaar
```

Combines Claude Code skills with a persistent CLI-backed task graph. Tasks live in
`.ctx/GODMODE.tasks.yaml` (gitignored) across sessions via causal `depends_on` chains.

**Skills:**

| Skill                                    | When to invoke                              |
| ---------------------------------------- | ------------------------------------------- |
| `godmode:using-godmode`                  | Session orientation, available skills       |
| `godmode:test-driven-development`        | Any feature or fix                          |
| `godmode:systematic-debugging`           | Any bug, test failure, unexpected behavior  |
| `godmode:brainstorming`                  | Before creative or design work              |
| `godmode:writing-plans`                  | Multi-step task with a spec                 |
| `godmode:verification-before-completion` | Before claiming work is done                |
| `godmode:task-management`                | Creating, tracking, executing a task graph  |
| `godmode:parallel-agents`                | 2+ independent tasks to run concurrently    |
| `godmode:code-review`                    | Quality pass before merge                   |
| `godmode:refactoring`                    | Restructure code without changing behaviour |
| `godmode:receiving-review`               | Process incoming review feedback            |

**Agent:** `tdd-crate-agent` — TDD implementation in a single Rust crate.

**Key CLI commands:**

```bash
godmode handon                        # triage at session start
godmode handoff                       # validate at session end
godmode task list
godmode task add <id> <title> [--depends-on t1,t2] [--crate-name <name>]
godmode task start <id>
godmode task done <id> [--commit <sha>]
godmode task next                     # show next runnable task(s)
godmode task pull                     # import pending doob todos as tasks
godmode plan ingest <plan.md>         # parse plan headings into task graph
godmode dispatch [--max 5]            # emit parallel agent chains as JSON
godmode status                        # counts + next runnable
```

**Workflow:**

```
brainstorming → writing-plans → plan ingest → handon
  → task next → task start → [tdd] → task done → task next
  → dispatch → parallel-agents → verification-before-completion → handoff
```

---

## valerie

```bash
claude plugin install valerie@bazaar
```

Requires `doob` on PATH.

**Skill:** `valerie` — triggers on: "add a todo", "what should I work on", "capture from
HANDOFF", "audit my todos".

**Workflows:**

1. Direct CRUD — add, list, complete, remove todos via doob
2. Council Report → Todos — parse devloop council output into prioritized todos
3. HANDOFF → Reconciliation — sync HANDOFF items into doob, avoid duplicates
4. HANDOFF Cleanup — prune closed items, write back capture status
5. Audit — cross-reference doob todos against HANDOFF, surface gaps

**Rule:** Valerie is the only skill that should touch `doob` or run GitHub issue sync.

---

## orca-strait

```bash
claude plugin install orca-strait@bazaar
```

**Prerequisites:** `gh` CLI authenticated, `cargo-nextest`, Rust workspace, `python3`.

**Usage:**

```
/orca-strait
/orca-strait /path/to/repo
/orca-strait --dry-run
```

Or trigger via prose: "implement the open issues in parallel using TDD agents".

**Workflow:** reads GitHub issues + HANDOFF files + plan → decomposes by crate → spawns
waves of `tdd-crate-agent` sub-agents (≤5 concurrent) → red/green/refactor/commit per crate
→ workspace-level `cargo nextest` + `cargo clippy`.

**Architecture enforcement:** hexagonal/SOLID — ports in domain layer, adapters in `infra/`,
in-memory trait doubles in tests, composition root is the only concrete adapter site.

**Hook:** `check-blocked` (PostToolUse/Agent) — surfaces `BLOCKED.md` immediately after any
agent call. Agents write `BLOCKED.md` and stop after 3 failed attempts.

---

## cannibalizer

```bash
claude plugin install cannibalizer@bazaar

# Also install the CLI
cargo install --path ~/dev/cannibalizer
```

4-stage pipeline: `scan → plan → gen → eat`

```bash
cnbl scan ~/some-repo --output scan.jsonl
cnbl plan --input scan.jsonl --repo-map repos.json --dry-run
cnbl gen --input plan.jsonl --out-dir cnbl-output
cnbl eat --input plan.jsonl --source-repo some-repo --dry-run
```

**Skill:** `cannibalizer` — orchestrates the full pipeline with approval gates at each step.

---

## Recommended Install Order

```bash
# 1. Register marketplace
claude plugin marketplace add https://github.com/89jobrien/bazaar

# 2. Secrets + session foundation
claude plugin install sanctum@bazaar

# 3. Dev workflow
claude plugin install atelier@bazaar

# 4. Task graph methodology (install CLI first)
cargo install --path ~/dev/godmode/crates/godmode-cli --root ~/.local
claude plugin install godmode@bazaar

# 5. Todo management (install doob first)
claude plugin install valerie@bazaar

# 6. Parallel TDD orchestration
claude plugin install orca-strait@bazaar

# 7. Repo absorption
cargo install --path ~/dev/cannibalizer
claude plugin install cannibalizer@bazaar
```

## Plugin Manifest Schema

Claude Code accepts only these fields in `.claude-plugin/plugin.json`:

```json
{
  "name": "plugin-name",
  "version": "1.0.0",
  "author": { "name": "Author Name" },
  "description": "One-line description"
}
```

Skills, agents, hooks, and commands are discovered by directory scan — do not declare them
in `plugin.json`. Extra fields (`skills`, `agents`, `homepage`, `repository`, `license`,
`keywords`) cause manifest validation failure.
