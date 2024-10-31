# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

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
