# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.4](https://github.com/build198x/build198x/compare/build198x-v0.2.3...build198x-v0.2.4) - 2026-08-28

### Other

- Ship installers and a Homebrew tap, like Asm198x has ([#31](https://github.com/build198x/build198x/pull/31))
- correct a capacity figure that mixed two kinds of kilobyte ([#24](https://github.com/build198x/build198x/pull/24))

## [0.2.3](https://github.com/build198x/build198x/compare/build198x-v0.2.2...build198x-v0.2.3) - 2026-08-27

### Fixed

- **`adf verify` called ordinary data disks corrupt.** Any disk whose
  boot-checksum field is zero was reported as `corrupt ADF: boot checksum`.
  AmigaDOS `Format` leaves that field zero until `Install` writes a bootstrap,
  so a plain data disk — including every disk `xdftool` produces — failed.
- **An ADF could only be filled halfway.** Mastering refused anything over about
  432K on an 880K floppy, because blocks were allocated upward from the root
  block and the lower half of the disk was never used. A single file now reaches
  about 865K — 98% of the media rather than 49%.

Both come from `format198x-commodore-amiga-adf` 0.3.0
([#22](https://github.com/build198x/build198x/pull/22)). Mastering output is
unchanged byte-for-byte.

### Other

- `mediaspec198x`, the media capability spec, is now published as its own crate
  rather than consumed from a git revision
  ([#17](https://github.com/build198x/build198x/pull/17)). Four orgs are told to
  consume it and cargo will not publish a crate that depends on a git revision,
  so the registry is the only route that serves them. No change to what this
  tool does.
- Tracks Rust 1.98.0 ([#21](https://github.com/build198x/build198x/pull/21)).

## [0.2.2](https://github.com/build198x/build198x/compare/build198x-v0.2.1...build198x-v0.2.2) - 2026-08-26

### Fixed

- consume the graduated format crates from crates.io, not a sibling path
- pin version alongside the format crate path deps

### Other

- follow the format crates to their org-prefixed names
- Merge branch 'main' into refactor/consume-format-crates
- consume the graduated format crates

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
