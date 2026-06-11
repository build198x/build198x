# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0](https://github.com/build198x/build198x/releases/tag/build198x-v0.1.0) - 2026-06-11

### Added

- per-constraint dither defaults - Floyd-Steinberg for free-palette targets
- add Emu198x smoke-fixture generation
- add the build198x image CLI
- add the spec-driven image conversion pipeline
- add SCR, Koala, Art Studio, and ILBM codecs

### Fixed

- resolve residual review findings
- apply code-review safe fixes
- render previews PAR-corrected so they show display proportions

### Other

- deduplicate determinism-sensitive kernels and spec-key the CLI gates
- scaffold workspace per the project-skeleton standard
