# Copilot Instructions — HeadlessCraft

> Authoritative. Follow every rule. If any rule is outdated or the codebase has drifted,
> update this file as part of the task.

---

## Before Any Task

1. Read the **[key ADRs](#key-adrs)** + any ADRs linked from the phase doc or touching your crate
2. Read the **relevant phase doc** (`docs/phases/phase-NN-*.md`) if task belongs to a phase
3. Read **[lifecycle docs](../docs/lifecycle/README.md)** when process questions arise

---

## Project Overview

**HeadlessCraft** — Rust framework for headless Minecraft Java Edition clients (bots, testing, automation).

- **Protocol:** MC 26.1 (version `775`, world `4786`)
- **Reference:** `mc-server-ref/decompiled/` — decompiled vanilla 26.1 JAR (gitignored)
- **Philosophy:** Wire-compatible with vanilla servers, idiomatic Rust internals, ergonomic API

---

## Workspace & Crate Dependencies

```
headlesscraft-types     ← no internal deps (shared coordinate types)
headlesscraft-nbt       ← no internal deps
headlesscraft-macros    ← no internal deps (proc-macro)
headlesscraft-protocol  ← types, nbt, macros
headlesscraft-world     ← types, nbt
headlesscraft-client    ← protocol, world, nbt
headlesscraft           ← client (public facade, re-exports)
```

**Never let a lower-layer crate import a higher-layer crate.**

Config files: `Cargo.toml` (workspace), `rustfmt.toml` (max_width=100), `deny.toml` (cargo-deny), `rust-toolchain.toml` (stable, MSRV 1.85).

---

## Lifecycle Rules

Follow the [Development Lifecycle](../docs/lifecycle/README.md): Identify → Research → **Arch Review Gate** → ADR → Plan → Test First → Implement → Review → Integrate → Retrospect.

- **Arch Review Gate (Stage 2.5):** Before planning or testing, question every constraining ADR. If outdated → create a superseding ADR first. Ask: Right pattern? Would a Rust dev choose this? Does it make sense for a client library? Will we regret this in 6 months?
- **CI:** After every push, wait for all jobs to pass. Never leave `main` broken.
- **Memories:** Update [memories.md](memories.md) after phases or when discovering gotchas.
- **Improvement:** Outdated ADRs → supersede. Better patterns → record + refactor. Missing tests → add now. Tech debt → TODO + memories.md.
- **Retrospective** after every phase → check memories.md for learnings, update it with new findings.

---

## Workflow

### Plan first when:

- New crate, module, or public trait
- Task touches >3 files
- Ambiguous request or multiple valid approaches
- Change affects a public trait

**Steps:** Explore Java ref + Rust code → plan + SQL todos → confirm with user.
**Skip planning for:** single-file fixes, typos, doc edits, dep bumps.

### Java Reference

Always read the equivalent Java class in `mc-server-ref/decompiled/net/minecraft/` first. Understand the algorithm, then **rewrite idiomatically** — never transliterate.

| Concern | Java path |
|---|---|
| Packets | `network/protocol/game/`, `login/`, etc. |
| Connection | `network/Connection.java`, `FriendlyByteBuf.java` |
| Client logic | `client/Minecraft.java`, `client/multiplayer/` |
| Chunks | `world/level/chunk/LevelChunk.java`, `LevelChunkSection.java` |
| Block states | `world/level/block/state/BlockBehaviour.java` |
| Entities | `world/entity/Entity.java`, `LivingEntity.java` |
| NBT | `nbt/CompoundTag.java`, `NbtIo.java` |
| Auth | `client/User.java`, Mojang API |

### Sub-Agent Dispatch

#### Built-in Agents

| Agent | Use for |
|---|---|
| `explore` | Quick codebase search, file discovery, answering structural questions |
| `task` | Build & test: `cargo test -p <crate>`, `cargo check --workspace` |
| `code-review` | General code review when custom agents aren't available |

#### Custom Agents (`.github/agents/`)

Prefer these over built-in agents — they have project-specific knowledge.

| Agent | File | Use for |
|---|---|---|
| `@rust-engineer` | `rust-engineer.md` | Rust implementation — features, bug fixes, refactoring across the workspace |
| `@java-reference` | `java-reference.md` | Analyze vanilla Java source in `mc-server-ref/decompiled/`, explain algorithms and protocol details |
| `@reviewer` | `reviewer.md` | Code review — ADR compliance, correctness, vanilla compatibility, performance |
| `@tester` | `tester.md` | Write tests — unit, integration, property-based, compliance, snapshots |
| `@docs-writer` | `docs-writer.md` | Write ADRs, phase docs, code documentation, update memories.md |
| `@vanilla-auditor` | `vanilla-auditor.md` | Vanilla compliance audit — compare HeadlessCraft behavior against decompiled Java source |

#### Dispatch Rules

- Parallelise independent `explore` and `@java-reference` calls.
- **Review↔Fix loop:** `@reviewer` flags issues → fix them → `@reviewer` re-reviews → repeat until clean pass.
- Use `@java-reference` before implementing any protocol/game logic to understand vanilla behavior first.
- Use `@tester` for test strategy and test writing.
- Use `@docs-writer` for ADRs, phase docs, and documentation updates.
- Use `@vanilla-auditor` for compliance audits — run before major releases or after implementing new protocol logic.
- **Do not delegate implementation to sub-agents** — use `@rust-engineer` for guidance, implement yourself.

### TDD Cycle

1. Write failing test → 2. Confirm failure (not compile-error) → 3. Implement minimum to pass → 4. Confirm green → 5. Refactor + re-run → 6. Code review + commit

**Test naming:** `test_<thing>_<condition>` or `<thing>_<outcome>_when_<condition>`

### Test Types

| Type | Location | When |
|------|----------|------|
| Unit | `#[cfg(test)] mod tests` | Every function |
| Integration | `crates/<crate>/tests/*.rs` | Cross-module, public API only |
| Property | inline or `tests/` (`proptest`) | All parsers, codecs, roundtrips |
| Compliance | `headlesscraft-protocol/tests/compliance.rs` | Protocol byte verification |
| Doc | `///` on public items | Every public item |
| Snapshot | `insta::assert_snapshot!` | Error messages, generated output |

**Minimum per PR:** Unit + Integration + Property-based (for parsers/codecs).
**Conventions:** `#[allow(clippy::unwrap_used, clippy::expect_used)]` in test modules. Integration = public API only. Proptest: `proptest_<thing>_<invariant>`. Doc examples: self-contained. Snapshots in `snapshots/` dirs.

### Before Every Commit

Grep for stale references after renames/moves:
```bash
grep -r "old_name" . --include="*.rs" --include="*.toml" --include="*.md"
```

---

## Rust Standards

- **Edition 2024**, stable toolchain, MSRV 1.85
- `#![warn(missing_docs)]` on library crates
- `#![deny(unsafe_code)]` unless justified with `SAFETY:` comment
- **Errors:** `thiserror` in libraries, `anyhow` only in examples/tests. Never `unwrap()`/`expect()` in production. Use `?` + `.context()` or `.map_err()`.
- **Naming:** Types `PascalCase`, functions `snake_case`, constants `SCREAMING_SNAKE`, modules `snake_case`, booleans `is_`/`has_`/`can_`, features `kebab-case`
- **Docs:** `///` on all public items with `# Errors` section when returning `Result`. Private helpers: `//` when non-obvious.
- **No magic numbers:** All protocol constants in a `constants` module or inline `const`.

### Async & Threading

- Network I/O: `tokio::net` with async reader/writer tasks
- Per-connection: reader + writer tasks with bounded `mpsc`
- Cross-thread: `tokio::sync::{mpsc, broadcast}`. Non-async locks: `parking_lot`. Concurrent maps: `dashmap::DashMap`

### Performance

- `ahash::AHashMap` for hot paths
- Avoid unnecessary allocations in packet processing hot paths
- Zero-copy parsing where possible with `bytes::Bytes`
- Support hundreds of concurrent client instances

---

## Key ADRs

Framework-level decisions that affect all code. All ADRs in `docs/adr/`.

Read the phase doc's "Architecture Decisions" section for domain-specific ADRs.
**New ADR when:** new crate/public trait, choosing between approaches, expensive-to-reverse decision.
**Lifecycle:** Proposed → Accepted → Superseded. Never edit accepted — create a superseding one.

---

## Versioning

This repo uses [Conventional Commits](https://www.conventionalcommits.org/) for automated versioning.

Format: `<type>(<scope>): <description>`
**Types:** `feat` (minor), `fix` (patch), `perf` (patch), `refactor`, `test`, `docs`, `chore`, `ci`
**Scopes:** `types`, `nbt`, `macros`, `protocol`, `world`, `client`, `ci`, `deps`
**Breaking:** `feat!:` + `BREAKING CHANGE:` in body. No `Co-authored-by:` trailers.
