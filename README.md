# bazaar

A Claude Code plugin marketplace and showcase generator.

## What it is

`bazaar` serves two purposes:

1. **Plugin marketplace** — a registry of Claude Code plugins installable via
   `claude plugin install <name>@bazaar`
2. **Showcase generator** — `bz`, a Rust binary that fetches project data from GitHub,
   crates.io, and PyPI and renders a static `index.html` + `README.md`

See [`examples/`](examples/) for sample generated output.

## Plugins

| Plugin         | Description                                                                       |
| -------------- | --------------------------------------------------------------------------------- |
| `atelier`      | Personal dev workflow — Rust gates, code review, CI, git safety, multi-repo pulse |
| `sanctum`      | 1Password auth and `.envrc` chain tracing                                         |
| `orca-strait`  | Parallel TDD orchestrator for Rust workspaces                                     |
| `valerie`      | Task and todo management — doob CLI integration, HANDOFF reconciliation           |
| `cannibalizer` | Absorb foreign repos — extract, classify, generate hexagonal components           |

### Install

Register the marketplace once per machine:

```sh
claude plugin marketplace add https://github.com/89jobrien/bazaar
```

Then install any plugin:

```sh
claude plugin install atelier@bazaar
claude plugin install sanctum@bazaar
claude plugin install orca-strait@bazaar
```

## `bz` — showcase generator

### Usage

```sh
bz [OPTIONS]

Options:
  --output <PATH>           Output HTML path [default: index.html]
  --readme <PATH>           Output Markdown path [default: README.md]
  --pypi-toml <PATH>        PyPI packages config [default: pypi.toml]
  --plugin-manifest <PATH>  Plugin manifest [default: .claude-plugin/marketplace.json]
  --readme-header <PATH>    Markdown header partial [default: README.header.md]
  --watch                   Re-generate on an interval (default: 300s)
  --interval <SECS>         Watch interval in seconds [default: 300]
```

### Environment variables

| Variable                | Required | Description                                   |
| ----------------------- | -------- | --------------------------------------------- |
| `BAZAAR_GITHUB_USER`    | yes      | GitHub username to fetch repos for            |
| `BAZAAR_CRATES_IO_USER` | yes      | crates.io username to fetch crates for        |
| `GITHUB_TOKEN`          | no       | GitHub API token (unauthenticated: 60 req/hr) |

### Build

```sh
cargo build --release -p bazaar-gen
```

### Local generation

```sh
mise run build   # build release binary
mise run bz      # one-shot generate → examples/
mise run watch   # continuous regeneration every 5 minutes
```

### CI

The [`generate` workflow](.github/workflows/generate.yml) runs on every push to `main`
and daily at 06:00 UTC. It builds `bz` from source, regenerates `examples/`, and commits
any changes.

## Releases

Binaries are published to [GitHub Releases](https://github.com/89jobrien/bazaar/releases)
on every `v*` tag for:

- `aarch64-apple-darwin` (macOS Apple Silicon)
- `x86_64-unknown-linux-musl` (Linux, statically linked)

```sh
git tag v0.x.0 && git push origin v0.x.0
```

## Repo maintenance

```sh
# Refresh repos.json (Rust repos updated in last 3 months)
bash scripts/fetch-repos.sh

# Archive a local project to SSD
bash scripts/archive-project.sh <project-name>
```
