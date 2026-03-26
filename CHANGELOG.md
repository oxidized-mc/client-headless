# Changelog

All notable changes to HeadlessCraft will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.1](https://github.com/dodoflix/HeadlessCraft/compare/v0.1.0...v0.1.1) (2026-03-26)


### 🐛 Bug Fixes

* add CFR fallback for VineFlower decompilation failures ([c24df2d](https://github.com/dodoflix/HeadlessCraft/commit/c24df2db5afed4320e8ff52ceb1f70c50def86b3))


### 🔨 Refactor

* restructure workspace from 7 crates to 3 ([884c224](https://github.com/dodoflix/HeadlessCraft/commit/884c224b2a426652d5a224c693a485f7e38e307d))

## [Unreleased]

### Added

- 3-crate Cargo workspace (headlesscraft, protocol, macros)
- Repository scaffolding (licenses MIT/Apache-2.0, CONTRIBUTING, CI)
- Rust tooling (rustfmt, clippy, cargo-deny, rust-toolchain)
- Copilot agents and prompts for development workflow
- GitHub Actions CI pipeline (lint, test, deny, MSRV)
