# Quality Gates — HeadlessCraft

Each lifecycle stage has gates that must pass before advancing.

## Gate Checklist

| Stage | Gate |
|-------|------|
| Research → Plan | Vanilla Java reference read and understood |
| Plan → Test | ADRs reviewed, plan confirmed with stakeholder |
| Test → Implement | Failing tests written (not compile errors) |
| Implement → Review | All tests green, `cargo check --workspace` passes |
| Review → Integrate | No blocker issues, CI green |
| Integrate → Retrospect | Main branch stable, no regressions |

## CI Requirements

All of these must pass before merge:

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo check --workspace --no-default-features`
- cargo-deny (licenses + advisories)
- MSRV check (Rust 1.85)
