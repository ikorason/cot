# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2](https://github.com/cot-rs/cot/compare/cot-v0.2.1...cot-v0.2.2) - 2025-04-03

### <!-- 2 -->Fixes

- don't show 404 when there are 0 objects in admin panel ([#270](https://github.com/cot-rs/cot/pull/270))

### <!-- 3 -->Other

- update `admin` example with a custom `TodoItem` model ([#270](https://github.com/cot-rs/cot/pull/270))

## [0.2.1](https://github.com/cot-rs/cot/compare/cot-v0.2.0...cot-v0.2.1) - 2025-03-30

### <!-- 2 -->Fixes

- *(cli)* migration generator not working in inner project directories ([#267](https://github.com/cot-rs/cot/pull/267))

### <!-- 3 -->Other

- use #[expect] instead of #[allow] where it makes sense ([#259](https://github.com/cot-rs/cot/pull/259))
- add #[diagnostic::on_unimplemented] for key traits ([#260](https://github.com/cot-rs/cot/pull/260))
- *(deps)* use database dependencies only with the "db" feature flag ([#264](https://github.com/cot-rs/cot/pull/264))

## [0.2.0](https://github.com/cot-rs/cot/compare/cot-v0.1.4...cot-v0.2.0) - 2025-03-25

### <!-- 0 -->Security

- cycle session ID on login, flush session on logout ([#246](https://github.com/cot-rs/cot/pull/246))

### <!-- 1 -->New features

- [**breaking**] use extractor pattern for request handlers ([#253](https://github.com/cot-rs/cot/pull/253)),
  introducing `FromRequest` and `FromRequestParts` traits and removing duplicated functionality from `RequestExt`
- add `SessionMiddleware` configuration ([#251](https://github.com/cot-rs/cot/pull/251))
- user-friendly message for `AddrInUse` error ([#233](https://github.com/cot-rs/cot/pull/233))
- support for "Remove Field" in automatic migration generator ([#232](https://github.com/cot-rs/cot/pull/232))
- support for "Remove Model" in Automatic Migration Generator ([#221](https://github.com/cot-rs/cot/pull/221))
- basic pagination support for admin panel ([#217](https://github.com/cot-rs/cot/pull/217))
- display object paths when (de)serialization error happened with serde
- add `RegisterAppsContext`, `AuthBackendContext`, `MiddlewareContext` as type aliases for `ProjectContext` in specific
  bootstrapping phases that are more semantic and whose names won't change when changing the phases

### <!-- 2 -->Fixes

- panic backtrace/location not displayed on the error page ([#237](https://github.com/cot-rs/cot/pull/237))
- include APP_NAME in model ([#228](https://github.com/cot-rs/cot/pull/228))

### <!-- 3 -->Other

- [**breaking**] upgrade edition to 2024 ([#244](https://github.com/cot-rs/cot/pull/244))
- [**breaking**] remove methods from the `RequestExt` that duplicate extractors' functionalities
- [**breaking**] `AuthRequestExt` trait is now replaced by `Auth` struct and `AuthMiddleware` is now required for
- [**breaking**] add `WithDatabase` bootstrapping phase
- `Urls` object can now be used with the `reverse!` macro and not only `Request`
- *(clippy)* add --all-targets to clippy CI and fix all warnings ([#240](https://github.com/cot-rs/cot/pull/240))
- add test for reverse!() reversing in the current app first ([#239](https://github.com/cot-rs/cot/pull/239))
- more docs (up to 100% doc coverage) ([#229](https://github.com/cot-rs/cot/pull/229))

## [0.1.4](https://github.com/cot-rs/cot/compare/cot-v0.1.3...cot-v0.1.4) - 2025-02-28

### Added

- add #[track_caller] to `unwrap`s for better panic messages (#212)

### Fixed

- use cot's version instead of cli's when creating a new project (#213)

## [0.1.3](https://github.com/cot-rs/cot/compare/cot-v0.1.2...cot-v0.1.3) - 2025-02-24

### Other

- add logo to the rustdoc (#198)

## [0.1.2](https://github.com/cot-rs/cot/compare/cot-v0.1.1...cot-v0.1.2) - 2025-02-23

### Added

- *(error)* enhance error logging with tracing integration (#186)

### Fixed

- switch back to using non-prerelease versions of crypto crates (#188)

### Other

- *(deps)* add info about dependencies to CONTRIBUTING.md (#192)

## [0.1.1](https://github.com/cot-rs/cot/compare/cot-v0.1.0...cot-v0.1.1) - 2025-02-21

### Other

- add README.md to the Cargo.toml metadata (#178)
- fix a typo in form.rs docs (#173)

## 0.1.0 - 2025-02-18

- initial version
