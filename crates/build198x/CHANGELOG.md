# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1](https://github.com/build198x/build198x/compare/build198x-v0.2.0...build198x-v0.2.1) - 2026-07-10

### Added

- *(adf)* mirror the create verb into the build198x adf subcommand
- *(adf)* mirror verify and info into the build198x adf subcommand
- add FFS support and fix a protection bug that broke KS2.0+ boots
- make the ADF master correct for any file size and name set
- build198x adf — from-scratch bootable OFS floppy master

### Other

- consume format-commodore-amiga-adf from crates.io, not the workspace
- move main.rs test module to end of file
- extract the Amiga ADF writer into its own crate

## [0.2.0](https://github.com/build198x/build198x/releases/tag/build198x-v0.2.0) - 2026-07-02

### Added

- add the beeper-phrase converter: notation in, audition WAV + phrase asm out (the audio lane's first tool, opened by `decisions/demand-gate-beeper-phrases.md`; calibrated by regenerating Gloaming's hand-authored phrase constants exactly)
- add `--repeat`: loop-point audition for title-screen phrases

### Fixed

- repair the Release-plz pipeline: the mediaspec path dependency now carries a version requirement (Asm198x's pattern), and this release was cut by hand to replace the v0.1.0 baseline tag, whose manifest release-plz could not package

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
