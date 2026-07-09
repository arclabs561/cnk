# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.3] - 2026-07-09

### Changed

- Updated the compressed-postings example to use `postings 0.4`.

## [0.2.2] - 2026-07-07

### Changed

- Updated the compressed-postings example to use `postings 0.3`.

## [0.2.1] - 2026-07-05

### Changed

- Clarified that `RocCompressor` is direct-use behind `ans`, not selected by
  the default chooser.
- Shared the varint implementation used by the delta-varint codecs.
- Updated the optional `ans` dependency to 0.4.

## [0.2.0] - 2026-04-13

### Added

- Add compressed postings example with postings index
- Add auto chooser and versioned envelopes
- Add inline hints to varint encoding hot paths
