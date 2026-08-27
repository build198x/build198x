# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.3](https://github.com/build198x/build198x/compare/build198x-adf-v0.2.2...build198x-adf-v0.2.3) - 2026-08-27

### Fixed

- **`verify` called ordinary data disks corrupt.** Any disk whose boot-checksum
  field is zero was reported as `corrupt ADF: boot checksum`. AmigaDOS `Format`
  leaves that field zero until `Install` writes a bootstrap, so a plain data
  disk — including every disk `xdftool` produces — failed. A formatted disk
  with no bootstrap now verifies, and a disk that *does* carry a bootstrap must
  still have the right checksum.
- **A disk could only be filled halfway.** `master` and `create` refused
  anything over about 432 KB on an 880 KB floppy, because blocks were allocated
  upward from the root block and the whole lower half of the disk was never
  used. A DD floppy now takes about 886 KB — 98% of the media rather than 49%.

Both come from `format198x-commodore-amiga-adf` 0.3.0
([#22](https://github.com/build198x/build198x/pull/22)). Output is unchanged
byte-for-byte: the same executable mastered through the old and new versions
produces an identical image.

### Other

- `mediaspec198x`, the media capability spec, is now published as its own crate
  rather than consumed from a git revision
  ([#17](https://github.com/build198x/build198x/pull/17)). No change to what
  this tool does.
- Tracks Rust 1.98.0 ([#21](https://github.com/build198x/build198x/pull/21)).

## [0.2.2](https://github.com/build198x/build198x/compare/build198x-adf-v0.2.1...build198x-adf-v0.2.2) - 2026-08-26

### Added

- *(adf)* add create verb to build198x-adf (general Volume builder)
- *(adf)* add verify and info verbs to build198x-adf
- add FFS support and fix a protection bug that broke KS2.0+ boots
- add the standalone build198x-adf binary

### Other

- follow the format crates to their org-prefixed names
- release v0.2.1 ([#7](https://github.com/build198x/build198x/pull/7))
- *(adf)* describe build198x-adf as the full read/write ADF tool
- build198x-adf description + doc header reflect FFS support
- make build198x-adf publish-ready for crates.io
- consume format-commodore-amiga-adf from crates.io, not the workspace

## [0.2.1](https://github.com/build198x/build198x/compare/build198x-adf-v0.2.0...build198x-adf-v0.2.1) - 2026-07-10

### Added

- *(adf)* add create verb to build198x-adf (general Volume builder)
- *(adf)* add verify and info verbs to build198x-adf
- add FFS support and fix a protection bug that broke KS2.0+ boots
- add the standalone build198x-adf binary

### Other

- *(adf)* describe build198x-adf as the full read/write ADF tool
- build198x-adf description + doc header reflect FFS support
- make build198x-adf publish-ready for crates.io
- consume format-commodore-amiga-adf from crates.io, not the workspace
