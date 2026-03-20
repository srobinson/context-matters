# Changelog

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
