# Aranya COSMOS

This repository holds the various crates that make up the Aranya COSMOS 
integration project, which adapts the [main Aranya
project](https://github.com/aranya-project/aranya) into a [COSMOS plugin](https://github.com/matcala/openc3-cosmos-gate).


## Supported Platforms and Status

Currently, Aranya COSMOS has been tested on Apple Silicon macOS but is expected to work also for Linux.

Aranya COSMOS is in its early stages and is not yet ready for production use.

## Crates

- [`demo-esp32-s3`](crates/demo-esp32-s3/) - the Aranya example app
- [`aranya-embedded-config`](crates/aranya-embedded-config/) - the COMSOS plugin

All of these crates are organized into a workspace, but compiling esp32
projects from the root workspace will not work due esp32 projects requiring a
different toolchain (see [Setup](#setup) below). Please change directory into
the individual crates and read their `README`s before attempting any builds.

## Setup

Aranya and Aranya Embedded require a [rust development
environment](https://www.rust-lang.org/), but compiling esp32 projects requires
some extra setup beyond that. Please follow the instructions in [The Rust on
ESP Book](https://docs.esp-rs.org/book/installation/index.html) for installing
for both RISC-V and Xtensa targets.

## Contributing

Find information on contributing to the Aranya project in
[`CONTRIBUTING.md`](https://github.com/aranya-project/.github/blob/main/CONTRIBUTING.md).

## Maintainers

This repository is maintained by software engineers employed at
[SpiderOak](https://spideroak.com/).
