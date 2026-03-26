---
agent: 'vanilla-compliance-audit'
description: 'Fix vanilla compliance issues in the HeadlessCraft codebase.'
---

# Vanilla Compliance Audit

Audit the entire HeadlessCraft codebase for behavioral divergence from the vanilla Minecraft client, then fix every finding.

## Instructions

1. Use `@vanilla-auditor` to audit the codebase. It will discover implemented systems, compare them against the vanilla Java source, and return findings.
2. Review the audit report.
3. Plan fixes grouped by severity (🔴 → 🟡 → 🔵 → ⚪), one commit per logical fix.
4. Implement fixes yourself in that order. Add tests for every fix. Run `cargo check --workspace && cargo test --workspace` after each.

## What to Skip

- Systems in planned/future phases — `@vanilla-auditor` already filters these
- Architecture differences (internal structure can differ) — only wire behavior matters
- Style or formatting — only behavioral issues
