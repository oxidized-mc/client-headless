# Changelog

All notable changes to oxidized-client-headless will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.0](https://github.com/oxidized-mc/client-headless/compare/v0.1.0...v0.2.0) (2026-04-05)


### 🚀 Features

* **ci:** add cargo publish and dev publish workflows ([3b6520c](https://github.com/oxidized-mc/client-headless/commit/3b6520c7af1777ad5c3699798b84198a7032147c))
* **headlesscraft:** integrate shared oxidized-mc crate ecosystem ([8499fb4](https://github.com/oxidized-mc/client-headless/commit/8499fb4e2ce63b2aadae41be669a7b50a232144c))


### 🐛 Bug Fixes

* **ci:** add permissions to release-please caller ([f3d5d78](https://github.com/oxidized-mc/client-headless/commit/f3d5d78ee6a028dbf39ac2000d18ab15dc8344b2))
* **ci:** add strip-patches to inline CI workflows ([f7de32a](https://github.com/oxidized-mc/client-headless/commit/f7de32ae0472edb299d2ce49302592355c0d9a43))
* **client-headless:** use allow-org for git source allowlisting ([856d6ee](https://github.com/oxidized-mc/client-headless/commit/856d6ee0020e7b1025be6021a2d073aa7be05c04))
* **deps:** switch from git to version deps for crates.io publishing ([0f80583](https://github.com/oxidized-mc/client-headless/commit/0f80583056606c587a7128badd7cae9885a85571))


### 🔨 Refactor

* flatten workspace to single crate ([0b5561c](https://github.com/oxidized-mc/client-headless/commit/0b5561c495cd8951a669d9978f4017c72e827454))
* remove unnecessary re-export modules ([f41e3b6](https://github.com/oxidized-mc/client-headless/commit/f41e3b6898dc87c74b03fab7cea58cc3fb31479c))
* rename headlesscraft crates to oxidized-client-headless ([5cc9e20](https://github.com/oxidized-mc/client-headless/commit/5cc9e20175819d126f22ea79c1733f896b17573c))

## [Unreleased]

### Added

- 3-crate Cargo workspace (oxidized-client-headless, protocol, macros)
- Repository scaffolding (licenses MIT/Apache-2.0, CONTRIBUTING, CI)
- Rust tooling (rustfmt, clippy, cargo-deny, rust-toolchain)
- Copilot agents and prompts for development workflow
- GitHub Actions CI pipeline (lint, test, deny, MSRV)
