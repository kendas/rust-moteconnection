# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
### Changed

- All messages sent during a disconnection are now dropped.
- The `AMReceiver` now emits `Event<Message>`-s instead of `Message`-s.
  Therefore, the `Event::Connected` and `Event::Disconnected` can be used
  by the user.

### Deprecated
### Removed
### Fixed

- The `Transport`-s are now able to cleanly exit when no connection could
  be established.

### Security

## [0.1.0] - 2020-08-02

### Added

- Serial-Forwarder protocol support
- Serial protocol support
- Raw packet dispatcher
- Active Message dispatcher
- `amlistener` example application

[Unreleased]: https://github.com/kendas/rust-moteconnection/compare/0.1.0...dev
[0.1.0]: https://github.com/kendas/rust-moteconnection/releases/tag/0.1.0
