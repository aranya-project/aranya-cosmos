# Aranya COSMOS Gate Demo

This example provisions two Aranya instances: a ground-side command gate and a flight-side policy enforcer.  
For a full OpenC3 COSMOS commanding walkthrough, see: https://github.com/matcala/openc3-cosmos-gate.git

## Prerequisites
1. rustup installed (https://rustup.rs/)
2. A local clone of this repository

## Running the Example

This example includes two binaries:
- `cosmos-gate-init`: initializes and onboards two Aranya instances, persisting their state
- `cosmos-gate-server`: loads a pre-initialized ground Aranya instance and exposes a REST API for OpenC3 COSMOS telecommand packets

### 1) Build the Aranya daemon
From the repository root:
```
cargo build -p aranya-daemon --release --features=aqc,afc,preview,experimental
```

### 2) Initialize Aranya instances
Change directory to enter this example's path and create two directories, which will hold the daemons' data.
```
mkdir gate-daemon cfs-daemon
```

Run `cosmos-gate-init` to initialize both a ground and flight Aranya instance:
```
cargo run --bin cosmos-gate-init <path_to_aranya-daemon_binary> <path_to_gate_daemon_work_dir> <path_to_flight_daemon_work_dir>
```
The Aranya daemon binary is located at `PROJECT_ROOT/target/release/aranya-daemon`.  
The default policy is applied, resulting in:
- A team created by the owner, with the member added to the team
- The ground Aranya instance as the owner (team creator)
- The flight Aranya instance as the member (joins the owner's team)

This process creates two Aranya daemon working directories, persisting the onboarding state.  
These directories can be exported and reused to retain initialization and onboarding memory.  
This allows quick onboarding for pre-initialized Aranya instances in target environments, such as a cFS flight software instance.

### 3) Start the server
Run `cosmos-gate-server` to expose the REST API for the COSMOS plugin:
```
cargo run --bin cosmos-gate-server <path_to_aranya-daemon_binary> <path_to_gate_daemon_work_dir>
```
If successful, `cosmos-gate-server` will listen on the default localhost address.

You are now ready to send commands through OpenC3 COSMOS to the target.  
The dispatcher script sends telecommand packets to this REST API, which validates and forwards them.

> Note: Currently, the default Aranya policy does not actually enforce authorization on commands.  
No RBAC or ABAC is applied to outgoing packets. Commands are always accepted, and the Aranya gate returns serialized commands for the flight-side Aranya instance to execute locally.  
To customize this behavior, update the `policy.md` file and rebuild the Aranya daemon.  
For more information, see the [Aranya policy documentation](https://github.com/aranya-project/aranya-docs/blob/main/docs/policy-v1.md) and [policy specification](https://github.com/aranya-project/aranya-docs/blob/main/docs/policy-v1.md).

