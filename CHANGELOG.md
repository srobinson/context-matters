# Changelog

## [0.2.8](https://github.com/srobinson/context-matters/compare/v0.2.7...v0.2.8) (2026-04-20)


### Bug Fixes

* **npm:** strip leading dir when extracting cargo-dist tarball ([#43](https://github.com/srobinson/context-matters/issues/43)) ([c5a2460](https://github.com/srobinson/context-matters/commit/c5a2460b1fa85042e937cee12c361973ed0a8852))

## [0.2.7](https://github.com/srobinson/context-matters/compare/v0.2.6...v0.2.7) (2026-04-20)


### Features

* **cli:** world-class CLI parity with MCP via cm-capabilities ([#41](https://github.com/srobinson/context-matters/issues/41)) ([0c09fb6](https://github.com/srobinson/context-matters/commit/0c09fb688df5c21d46160ff72883fc270e91082e))

## [0.2.6](https://github.com/srobinson/context-matters/compare/v0.2.5...v0.2.6) (2026-04-20)


### Features

* smart browse infer local scope for cx_browse ([77a6820](https://github.com/srobinson/context-matters/commit/77a6820b5b41f08e4a3512e15c2364b8b07e4411))

## [0.2.5](https://github.com/srobinson/context-matters/compare/v0.2.4...v0.2.5) (2026-04-15)


### Bug Fixes

* **release:** add missing [profile.dist] for cargo-dist builds ([#37](https://github.com/srobinson/context-matters/issues/37)) ([e3ae040](https://github.com/srobinson/context-matters/commit/e3ae0409f438b7118b0a9699d0e465dc3c6c623b))

## [0.2.4](https://github.com/srobinson/context-matters/compare/v0.2.3...v0.2.4) (2026-04-15)


### Features

* **release:** swap hand-rolled matrix for cargo-dist ([#35](https://github.com/srobinson/context-matters/issues/35)) ([23adb5d](https://github.com/srobinson/context-matters/commit/23adb5d493b68cd4a4f4eab1d9a7c4098ddb546c))

## [0.2.3](https://github.com/srobinson/context-matters/compare/v0.2.2...v0.2.3) (2026-04-12)


### Bug Fixes

* **cx:** utf-8 boundary crash in insert_highlights + mcp panic boundary ([#33](https://github.com/srobinson/context-matters/issues/33)) ([d6a054e](https://github.com/srobinson/context-matters/commit/d6a054e1a3e72be6952077ac2c544d3c58391e50))

## [0.2.2](https://github.com/srobinson/context-matters/compare/v0.2.1...v0.2.2) (2026-04-11)


### Bug Fixes

* **cx:** prefix-tier crash + short_id rip-out (ALP-1764) ([#31](https://github.com/srobinson/context-matters/issues/31)) ([de67d55](https://github.com/srobinson/context-matters/commit/de67d554cbcf80bc3dfa973472f8ef43041d306f))

## [0.2.1](https://github.com/srobinson/context-matters/compare/v0.2.0...v0.2.1) (2026-04-11)


### Features

* improved retrieval (ALP-1745) ([bb72143](https://github.com/srobinson/context-matters/commit/bb72143a71f8953fcc763204c406c25c96fd6556))

## [0.2.0](https://github.com/srobinson/context-matters/compare/v0.1.12...v0.2.0) (2026-04-11)


### ⚠ BREAKING CHANGES

* redesign cx_* MCP response payloads to YAML text ([#27](https://github.com/srobinson/context-matters/issues/27))

### Features

* redesign cx_* MCP response payloads to YAML text ([#27](https://github.com/srobinson/context-matters/issues/27)) ([859bc11](https://github.com/srobinson/context-matters/commit/859bc11d18316e329b07e3c5c89c6e500a782178))

## [0.1.12](https://github.com/srobinson/context-matters/compare/v0.1.11...v0.1.12) (2026-03-21)


### Bug Fixes

* align config management with helioy ecosystem standard ([#20](https://github.com/srobinson/context-matters/issues/20)) ([e85e3c2](https://github.com/srobinson/context-matters/commit/e85e3c26a014d77399c8fe3ea7b4ca4ff34b8f38))

## [0.1.11](https://github.com/srobinson/context-matters/compare/v0.1.10...v0.1.11) (2026-03-20)


### Bug Fixes

* improve array parameter docs and error messages ([759e511](https://github.com/srobinson/context-matters/commit/759e5110d2b6b5f7bc47c9bb12a45e010866bd1d))
* improve cx_recall scope metadata and per-entry token estimates ([4a628c0](https://github.com/srobinson/context-matters/commit/4a628c0bf4c1dc53725ec4a4265ee99ab1c855a9))

## [0.1.10](https://github.com/srobinson/context-matters/compare/v0.1.9...v0.1.10) (2026-03-20)


### Bug Fixes

* improve cx_recall query guidance and add zero-result hints ([#17](https://github.com/srobinson/context-matters/issues/17)) ([a2a1382](https://github.com/srobinson/context-matters/commit/a2a138292b91e2c4bc6ccf11efb568f016e57c06))

## [0.1.9](https://github.com/srobinson/context-matters/compare/v0.1.8...v0.1.9) (2026-03-20)


### Features

* extract shared capabilities into cm-capabilities crate ([#15](https://github.com/srobinson/context-matters/issues/15)) ([a1a49d0](https://github.com/srobinson/context-matters/commit/a1a49d0cf00088cefb51ee58195658b38ebf7e73))

## [0.1.8](https://github.com/srobinson/context-matters/compare/v0.1.7...v0.1.8) (2026-03-19)


### Features

* **cm-web:** Context Store Monitor and Curator ([#13](https://github.com/srobinson/context-matters/issues/13)) ([1e33f6f](https://github.com/srobinson/context-matters/commit/1e33f6fae7953c37750665f5cc194557612fb665))


### Bug Fixes

* update Cargo.lock for v0.1.7 and sync lockfile in release workflow ([09439e3](https://github.com/srobinson/context-matters/commit/09439e3973f00f3988f6ccc0c25467c3c07b69fc))

## [0.1.7](https://github.com/srobinson/context-matters/compare/v0.1.6...v0.1.7) (2026-03-19)


### Features

* add mutation history infrastructure to cm-store ([#10](https://github.com/srobinson/context-matters/issues/10)) ([4ab747c](https://github.com/srobinson/context-matters/commit/4ab747c3663ed8f23f1fa122420ca216759b5b4b))

## [0.1.6](https://github.com/srobinson/context-matters/compare/v0.1.5...v0.1.6) (2026-03-18)


### Features

* migrate to native async traits and add comprehensive test infrastructure ([#8](https://github.com/srobinson/context-matters/issues/8)) ([ebe6214](https://github.com/srobinson/context-matters/commit/ebe6214038fae6c9cb322a3c7d9bca1fd4453af3))

## [0.1.5](https://github.com/srobinson/context-matters/compare/v0.1.4...v0.1.5) (2026-03-14)


### Features

* add tags to FTS5 index and cx_stats, per-exchange deposit titles ([#6](https://github.com/srobinson/context-matters/issues/6)) ([cca001a](https://github.com/srobinson/context-matters/commit/cca001a301245b5dbe59484400c35936d8599bc2))

## [0.1.4](https://github.com/srobinson/context-matters/compare/v0.1.3...v0.1.4) (2026-03-14)


### Bug Fixes

* cx_recall without scope searches all entries instead of only global ([14e382f](https://github.com/srobinson/context-matters/commit/14e382fae58d2eaf8fc8ac5fa5ef0f5e1f024dd3))

## [0.1.3](https://github.com/srobinson/context-matters/compare/v0.1.2...v0.1.3) (2026-03-14)


### Bug Fixes

* restructure npm scoped package layout and add Windows build target ([27f137c](https://github.com/srobinson/context-matters/commit/27f137cef89db167419b74db7d630294e50faf2e))

## [0.1.2](https://github.com/srobinson/context-matters/compare/v0.1.1...v0.1.2) (2026-03-14)


### Bug Fixes

* sanitize FTS5 query input to match unicode61 tokenizer behavior ([df4947c](https://github.com/srobinson/context-matters/commit/df4947cf5912ded2109372ff9a870645e93d92cd))

## [0.1.1](https://github.com/srobinson/context-matters/compare/v0.1.0...v0.1.1) (2026-03-14)


### Features

* implement MCP server, context store, and 9 cx_* tools ([#1](https://github.com/srobinson/context-matters/issues/1)) ([b26dde5](https://github.com/srobinson/context-matters/commit/b26dde5923dc23159560bb3c307b5b3fd6b9f239))
