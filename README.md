# Aranya COSMOS

A shared foundation repository providing Aranya policy, daemon configuration, and integration apps used by multiple demo projects as a Git submodule.

This repo originated as a standalone [OpenC3 COSMOS](https://docs.openc3.com/docs) demo. That demo has since been consolidated into [aranya-space-demo](https://github.com/aranya-project/aranya-space-demo). The COSMOS plugin is now maintained at the org-hosted [cosmos-aranya-gate](https://github.com/aranya-project/cosmos-aranya-gate) repository.

## Repository Structure

- **crates/aranya-daemon** -- Aranya daemon with policy, key management, and sync
- **crates/aranya-client** -- Rust client library for applications
- **crates/aranya-client-capi** -- C API bindings for the client library
- **crates/aranya-daemon-api** -- API definitions for daemon communication
- **crates/aranya-keygen** -- Key generation utilities
- **crates/aranya-util** -- Common utilities
- **examples/rust/cosmos-gate** -- Integration apps (initializer + REST server) used across demos

## Branch-per-Integration Pattern

Downstream demo projects include this repository as a submodule and point to specific branches that tailor the code and policy for their integration scenario:

| Branch | Purpose |
|--------|---------|
| `main` | Shared baseline with default policy and apps |
| `feat/mavlink-cosmos-gate` | Adds MAVLink-specific policy rules and new Aranya APIs for UAV command-and-control, used by [aranya-uav-demo](https://github.com/aranya-project/aranya-uav-demo) |
| `gateway-deny-scenario` | Small policy modification to demonstrate command denial, used by [aranya-gateway-demo](https://github.com/aranya-project/aranya-gateway-demo) |

## Downstream Demo Projects

The following projects consume this repository as a submodule:

- [**aranya-space-demo**](https://github.com/aranya-project/aranya-space-demo) -- NASA cFS + OpenC3 COSMOS full demo (the original COSMOS demo was consolidated here)
- [**aranya-gateway-demo**](https://github.com/aranya-project/aranya-gateway-demo) -- NASA cFS + OpenC3 COSMOS full demo, with Aranya deployed as a network gateway rather than as cFS app
- [**aranya-uav-demo**](https://github.com/aranya-project/aranya-uav-demo) -- PX4 + MAVLink UAV demo
- [**aranya-gateway-standalone**](https://github.com/aranya-project/aranya-gateway-standalone) -- Standalone CCSDS/MAVLink/UDP network gateway demo

## Build Instructions

### Build the Aranya daemon

```bash
cargo build -p aranya-daemon --release --features=afc,preview,experimental
```

The binary is placed at `target/release/aranya-daemon`.

### Build all crates

```bash
cargo build --release
# or
cargo make build
```

### Run tests

```bash
cargo make test
```

### Code quality

```bash
cargo make correctness   # formatting, clippy, feature checks
```

## Supported Platforms

Tested on Apple Silicon macOS. Expected to work on Linux. Windows is not supported.

## License Notice

This project integrates with OpenC3 COSMOS (licensed under AGPL-3.0) but does not include, redistribute, or modify COSMOS. Users must obtain COSMOS separately and comply with its license. All components provided here are independent works that interact with COSMOS only through its documented plugin, configuration, and protocol interfaces. Use with commercial or Enterprise editions of COSMOS is subject to the applicable OpenC3 license agreement.

## Maintainers

The Aranya+COSMOS integration is led by [Matteo Calabrese](https://github.com/matcala).
The Aranya project, on which this project is based, is maintained by software engineers at
[SpiderOak](https://spideroak.com/).
