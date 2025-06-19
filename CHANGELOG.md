# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## [0.3.5](https://github.com/Stedi/jsonata-rs/compare/v0.3.4...v0.3.5) - 2025-03-31

### Fixed

- *(deps)* update rust crate rand to 0.9.0 ([#162](https://github.com/Stedi/jsonata-rs/pull/162))

### Other

- *(deps)* update github actions upgrade ([#170](https://github.com/Stedi/jsonata-rs/pull/170))
- *(deps)* update github actions upgrade ([#169](https://github.com/Stedi/jsonata-rs/pull/169))
- *(deps)* update github actions upgrade ([#168](https://github.com/Stedi/jsonata-rs/pull/168))
- *(deps)* update github actions upgrade ([#166](https://github.com/Stedi/jsonata-rs/pull/166))
- *(deps)* update rust crate regress to v0.10.3 ([#167](https://github.com/Stedi/jsonata-rs/pull/167))
- *(deps)* update github actions upgrade ([#161](https://github.com/Stedi/jsonata-rs/pull/161))
- *(deps)* pin dependencies ([#160](https://github.com/Stedi/jsonata-rs/pull/160))
- add failing test case for  syntax ([#163](https://github.com/Stedi/jsonata-rs/pull/163))

## [0.3.4](https://github.com/Stedi/jsonata-rs/compare/v0.3.3...v0.3.4) - 2025-02-03

### Fixed

- reducing should eagerly evaluate thunks (#157)
- wasm32-wasi has been deprecated (#158)

### Other

- *(deps)* update github actions upgrade (#150)

## [0.3.3](https://github.com/Stedi/jsonata-rs/compare/v0.3.2...v0.3.3) - 2024-12-12

### Fixed

- reduce stack memory usage by boxing more regex types ([#153](https://github.com/Stedi/jsonata-rs/pull/153))

## [0.3.2](https://github.com/Stedi/jsonata-rs/compare/v0.3.1...v0.3.2) - 2024-12-11

### Fixed

- reduce memory footprint of TokenKind from 120 to 32 bytes ([#151](https://github.com/Stedi/jsonata-rs/pull/151))

### Other

- *(deps)* update github actions upgrade ([#148](https://github.com/Stedi/jsonata-rs/pull/148))

## [0.3.1](https://github.com/Stedi/jsonata-rs/compare/v0.3.0...v0.3.1) - 2024-11-20

### Fixed

- handle year fmt in from millis fn ([#144](https://github.com/Stedi/jsonata-rs/pull/144))

### Other

- *(deps)* update github actions upgrade ([#143](https://github.com/Stedi/jsonata-rs/pull/143))

## [0.3.0](https://github.com/Stedi/jsonata-rs/compare/v0.2.0...v0.3.0) - 2024-11-18

### Added

- Add support for regex expressions ([#125](https://github.com/Stedi/jsonata-rs/pull/125))
- Handle regex pattern for Value type ([#118](https://github.com/Stedi/jsonata-rs/pull/118))

### Fixed

- $millis should return a number ([#142](https://github.com/Stedi/jsonata-rs/pull/142))
- some type-errors in function calls caused panics ([#141](https://github.com/Stedi/jsonata-rs/pull/141))

### Other

- bump up actions workflow in osv scanner ([#138](https://github.com/Stedi/jsonata-rs/pull/138))
- pin dependency fix for rust toolchain ([#137](https://github.com/Stedi/jsonata-rs/pull/137))
- *(deps)* pin dependencies ([#136](https://github.com/Stedi/jsonata-rs/pull/136))
- use Renovate best practices ([#135](https://github.com/Stedi/jsonata-rs/pull/135))
- *(deps)* update taiki-e/install-action digest to 9c04113 ([#128](https://github.com/Stedi/jsonata-rs/pull/128))
- add permissions definitions at workflow and jobs level ([#129](https://github.com/Stedi/jsonata-rs/pull/129))
- Revert "chore: exempt CHANGELOG from codeowners ([#121](https://github.com/Stedi/jsonata-rs/pull/121))" ([#124](https://github.com/Stedi/jsonata-rs/pull/124))
- Update scorecard.yml ([#123](https://github.com/Stedi/jsonata-rs/pull/123))
- run tests within merge queue ([#122](https://github.com/Stedi/jsonata-rs/pull/122))
- exempt CHANGELOG from codeowners ([#121](https://github.com/Stedi/jsonata-rs/pull/121))
- *(deps)* update github actions upgrade ([#119](https://github.com/Stedi/jsonata-rs/pull/119))

## [0.2.0](https://github.com/Stedi/jsonata-rs/compare/v0.1.10...v0.2.0) - 2024-11-01

### Added

- support additional functions ([#110](https://github.com/Stedi/jsonata-rs/pull/110))
- support parsing regex literals ([#111](https://github.com/Stedi/jsonata-rs/pull/111))

### Fixed

- used steps in the wrong order ([#117](https://github.com/Stedi/jsonata-rs/pull/117))
- use app token when checking out repo so that PR triggers will run ([#116](https://github.com/Stedi/jsonata-rs/pull/116))

### Other

- github app ([#115](https://github.com/Stedi/jsonata-rs/pull/115))
- remove github token from action ([#113](https://github.com/Stedi/jsonata-rs/pull/113))
- add non_exhaustive to the Error enum ([#112](https://github.com/Stedi/jsonata-rs/pull/112))
- create CODEOWNERS; add security policy ([#109](https://github.com/Stedi/jsonata-rs/pull/109))
- add OSV-scanner ([#107](https://github.com/Stedi/jsonata-rs/pull/107))
- add OSSF Scorecard and badge ([#105](https://github.com/Stedi/jsonata-rs/pull/105))
- update secrets env var ([#103](https://github.com/Stedi/jsonata-rs/pull/103))

## [0.1.10](https://github.com/Stedi/jsonata-rs/compare/v0.1.9...v0.1.10) - 2024-10-31

### Added

- support a custom fn to handle regex pattern ([#97](https://github.com/Stedi/jsonata-rs/pull/97))

### Fixed

- using the default token prevents the release PR from having PR checks run ([#101](https://github.com/Stedi/jsonata-rs/pull/101))

### Other

- Automate Release Pipeline || Test Token Update ([#94](https://github.com/Stedi/jsonata-rs/pull/94))
- clean up README.md and status.md ([#99](https://github.com/Stedi/jsonata-rs/pull/99))
- add release-plz workflow ([#98](https://github.com/Stedi/jsonata-rs/pull/98))

### Added

- Context and positional binds
- `$sort` and `$join` functions

## [0.0.0] - 2022-05-28

Initial version published to crates.io.

## [0.1.2] - 2024-04-28

Adding support for base64 encoding decoding, thanks to nated0g for the contribution.

## [0.1.3] - 2024-05-09

- Support for `jsonata.register_function`
- Support for evaluation with bindings

## [0.1.4] - 2024-06-24

- Enhancements to improve performance

## [0.1.5] - 2024-06-26

- Enhancements to improve performance

## [0.1.6] - 2024-09-09

- `$random`, `$round` and `$distinct` function support.

## [0.1.7] - 2024-09-13

- `$fromMillis`, `$toMillis`, `$substringBefore`, `$substringAfter`, `$now` function support.

## [0.1.8] - 2024-09-18

- `$reduce` function support.
