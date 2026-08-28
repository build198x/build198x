# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.5](https://github.com/build198x/build198x/compare/build198x-adf-v0.2.4...build198x-adf-v0.2.5) - 2026-08-28

### Added

- **Prebuilt binaries and a Homebrew formula — the first release to carry
  either.** Until now this crate rode the CLI's `build198x-vX.Y.Z` tag, and
  that tag names a package, so cargo-dist built that one package and dropped
  the rest of the workspace. Two releases shipped with no adf archives at all
  ([#34](https://github.com/build198x/build198x/issues/34)). It tags itself
  now ([#37](https://github.com/build198x/build198x/pull/37)):

  ```
  brew install build198x/homebrew-tap/build198x-adf
  ```

  Shell and PowerShell installers ship beside the archives, and
  `cargo install build198x-adf` works as it always has. The tool itself is
  unchanged — this release is how you get it, not what it does.

## [0.2.4](https://github.com/build198x/build198x/compare/build198x-adf-v0.2.3...build198x-adf-v0.2.4) - 2026-08-28

### Other

- **Version bump only.** The release that added installers and a Homebrew tap
  ([#31](https://github.com/build198x/build198x/pull/31)) built `build198x`
  alone: the `build198x-v0.2.4` tag names a package, so cargo-dist announced
  that package and nothing else in the workspace
  ([#34](https://github.com/build198x/build198x/issues/34)). This binary has no
  archives and no formula in that release.

  `cargo install build198x-adf` installs 0.2.4 as usual, and `build198x adf` —
  the taught path — is in the `build198x` archives and formula.

## [0.2.3](https://github.com/build198x/build198x/compare/build198x-adf-v0.2.2...build198x-adf-v0.2.3) - 2026-08-27

### Fixed

- **`verify` called ordinary data disks corrupt.** Any disk whose boot-checksum
  field is zero was reported as `corrupt ADF: boot checksum`. AmigaDOS `Format`
  leaves that field zero until `Install` writes a bootstrap, so a plain data
  disk — including every disk `xdftool` produces — failed. A formatted disk
  with no bootstrap now verifies, and a disk that *does* carry a bootstrap must
  still have the right checksum.
- **A disk could only be filled halfway.** `master` and `create` refused
  anything over about 432K on an 880K floppy, because blocks were allocated
  upward from the root block and the whole lower half of the disk was never
  used. A single file now reaches about 865K — 98% of the media rather than 49%.

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
