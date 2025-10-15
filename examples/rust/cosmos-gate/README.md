# Aranya COSMOS Gate Demo

This example provisions two Aranya instances, a ground-side command gate and a flight-side policy enforcer, then exposes a REST API that an OpenC3 COSMOS plugin can post telecommand packets to for validation and serialization.

For a full OpenC3 COSMOS walkthrough, see the companion plugin repository: https://github.com/matcala/openc3-cosmos-gate.git

## Prerequisites

1. [rustup](https://rustup.rs/) installed
2. A local clone of this repository
3. Cargo available in your shell, verify with `cargo --version`

## Components

- **aranya-daemon**  
  Long-running process that maintains state and evaluates policy.

- **cosmos-gate-init**  
  One-time initializer that creates two Aranya working directories, one for the ground gate, one for the flight enforcer, and onboards them into a team.

- **cosmos-gate-server**  
  Thin REST server that loads the pre-initialized ground Aranya instance and exposes an HTTP endpoint for COSMOS telecommand packets.

## Quick Start

### 1) Build the Aranya daemon

From the repository root:

```bash
cargo build -p aranya-daemon --release --features=aqc,afc,preview,experimental
```

The daemon binary will be at:

```
PROJECT_ROOT/target/release/aranya-daemon
```

### 2) Initialize the ground and flight Aranya instances

Create two working directories for persistent state, then run the initializer:

```bash
mkdir -p PROJECT_ROOT/examples/rust/cosmos-gate/gate-daemon
mkdir -p PROJECT_ROOT/examples/rust/cosmos-gate/flight-daemon

cd PROJECT_ROOT/examples/rust/cosmos-gate
```

```bash
cargo run --bin cosmos-gate-init <path_to_aranya-daemon_binary> <path_to_gate_daemon_dir> <path_to_flight_daemon_dir>
```

What this does:

- Creates a team owned by the ground instance, adds the flight instance as a member
- Persists onboarding state inside the `gate-daemon` and `flight-daemon` directories
- Produces ready-to-ship working dirs that you can reuse on other machines to skip re-onboarding

### 3) Start the ground REST server

Run the server and point it at the ground working directory:

```bash
cargo run --bin cosmos-gate-server  <path_to_aranya-daemon_binary> <path_to_gate_daemon_dir>
```

If successful, the server listens on `127.0.0.1` using its default port. Use this URL as the `rest_endpoint` in your COSMOS dispatcher configuration.

## How It Works

1. COSMOS sends a telecommand through your custom WRITE protocol to a dispatcher script.
2. The dispatcher POSTs the packet bytes to the `cosmos-gate-server` REST API.
3. The server asks the ground Aranya instance to validate and serialize the packet.
4. The serialized command is returned to the dispatcher, which forwards it to the target via COSMOS.

## Policy Behavior

By default, the provided policy accepts all commands. No RBAC or ABAC is enforced, commands are always accepted, and a serialized payload is returned for the flight-side Aranya to execute locally.

To enforce authorization, edit `policy.md` inside the daemon source, rebuild `aranya-daemon`, then re-initialize or update your working directories. See:

- Policy documentation: https://aranya-project.github.io/core-concepts/policy
- Policy specification: https://github.com/aranya-project/aranya-docs/blob/main/docs/policy-v1.md

## Paths Summary

- Daemon binary: `PROJECT_ROOT/target/release/aranya-daemon`
- Ground working dir: `PROJECT_ROOT/examples/rust/cosmos-gate/gate-daemon`
- Flight working dir: `PROJECT_ROOT/examples/rust/cosmos-gate/flight-daemon`

You can archive and reuse these working directories to preserve onboarding state.

## Troubleshooting

- **Server not listening on localhost**
  - Confirm the daemon path is correct.
  - Ensure `cosmos-gate-init` was run successfully and `gate-daemon` contains state files.

- **COSMOS cannot reach the REST API**
  - From inside the COSMOS container, verify routing to the host, for example `curl http://host.docker.internal:PORT/health` on macOS, or map the host IP on Linux.

- **Policy changes have no effect**
  - Rebuild `aranya-daemon` after editing policy.

## Reset or Start Fresh
To reset state, stop the server and remove the working directories, then repeat the initialization step:

```bash
rm -rf PROJECT_ROOT/examples/rust/cosmos-gate/gate-daemon PROJECT_ROOT/examples/rust/cosmos-gate/flight-daemon

mkdir -p PROJECT_ROOT/examples/rust/cosmos-gate/gate-daemon
mkdir -p PROJECT_ROOT/examples/rust/cosmos-gate/flight-daemon

cd PROJECT_ROOT/examples/rust/cosmos-gate
```

```bash
cargo run --bin cosmos-gate-init  <path_to_aranya-daemon_binary> <path_to_gate_daemon_dir> <path_to_flight_daemon_dir>
```

## Next Steps
- Wire the REST endpoint into your COSMOS plugin dispatcher.
- Stand up a target and send a `NOOP` or simple command to validate the path.
- Introduce a policy that denies specific commands or restricts by user, role, or attribute.
