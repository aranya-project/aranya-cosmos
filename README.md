# Aranya COSMOS

This repository holds the various crates and external resources that make up the Aranya COSMOS integration project, that adapts the [main Aranya
project](https://github.com/aranya-project/aranya) into a [COSMOS plugin](https://github.com/matcala/openc3-cosmos-gate).

Aranya COSMOS consists of **three main components**:

- [**OpenC3 COSMOS**](https://docs.openc3.com/docs)
	- The cloud-native, containerized, microservice-oriented, command-and-control system maintained by [OpenC3](https://openc3.com/).

- [**Aranya `cosmos-gate`**](examples/rust/cosmos-gate)
	- The Aranya ground app crate.
	- Exposes a REST API and evaluates outgoing telecommands from the COSMOS plugin’s dispatcher against an Aranya policy.

- [**The `openc3-cosmos-gate` COSMOS plugin**](https://github.com/matcala/openc3-cosmos-gate)
	- Routes telecommands through a custom WRITE protocol to the dispatcher, which then posts to the Aranya gate via a REST API.

 ## Supported Platforms and Status

Currently, Aranya COSMOS has been tested on Apple Silicon macOS but is expected to also work on Linux. To better understand how to tweak the demo for a Linux environment, refer to the instructions in the [`openc3-cosmos-gate`](https://github.com/matcala/openc3-cosmos-gate) submodule.

> Aranya COSMOS is in its early stages and is not yet ready for production use.

## 2025 Space Software Summit Workshop and Demo

Aranya COSMOS was introduced at the [2025 Space Software Summit Workshop](https://www.spacesoftwaresummit.com/), and a brief demo was shared with participants. 

The steps below provide instructions for running a simplified version of the demo which provisions two Aranya instances, a ground-side command gate and a flight-side policy enforcer and exposes a REST API that an OpenC3 COSMOS plugin can post telecommand packets to for validation and serialization.

For more information about this demo, see the companion [plugin repository](https://github.com/matcala/openc3-cosmos-gate.git) which is included in this repo as a submodule.

### Prerequisites

> Tested on Apple Silicon macOS. Linux should behave similarly. Aranya does not support Windows. Running this on Windows would require additional reconfiguration. If you make it work on Windows, please let us know.

1. [Docker Desktop](https://docs.docker.com/get-started/get-docker/) or a Docker engine with Docker Compose
2. An OpenC3 COSMOS deployment, see the [installation guide](https://docs.openc3.com/docs/getting-started/installation)
    - You'll need to update `docker-compose.yaml` to allow inbound UDP into the `openc3-operator` container. Under `ports`, add:

    ```yaml
    - "127.0.0.1:6201:6201/udp"
    ```
	- Make sure to leave COSMOS running in the background. You can verify it is running by visiting [http://localhost:2900/](http://localhost:2900/).
3. Access to the COSMOS CLI, or the prebuilt gem `openc3-cosmos-gate-1.0.0.gem`
4. [rustup](https://rustup.rs/)
    - Verify cargo is available in your shell with `cargo --version`

If you feel lost at any point, see Helpers and Troubleshooting steps in the companion [plugin repository](https://github.com/matcala/openc3-cosmos-gate.git).


### Installation

1. Clone the aranya-cosmos repo:
	```bash 
	git clone https://github.com/aranya-project/aranya-cosmos.git
	``` 
      
2. Initialize the `openc3-cosmos-gate` plugin submodule: 
	```bash 
	git submodule update --init --remote
	```

### Build and Install the Plugin

1. Build the plugin with the COSMOS CLI to produce a `.gem`, or use the `openc3-cosmos-gate-1.0.0.gem` file provided in the `openc3-cosmos-gate` submodule.
    - To build with CLI, run the following from inside the submodule root:
      ```bash
      openc3.sh cli rake build VERSION=X.X.X  # e.g., 2.0.0
      ```
2. Install the plugin in COSMOS:
   - Open the Admin Console.
   - Click **Install From File**, select the `.gem` you built, or the prebuilt one.

3. Verify in CmdTlmServer:
   - Interface `GATE_INT` appears and shows **CONNECTED**.
   - Target `GATE` routes telecommands and telemetry through `GATE_INT`.

### Initialize the ground and flight Aranya instances

1. Build the Aranya daemon from the `aranya-cosmos` repository root:
	```bash
	cargo build -p aranya-daemon --release --features=aqc,afc,preview,experimental
	```
	This places the daemon binary at: `aranya-cosmos/target/release/aranya-daemon`

1. Access the COSMOS gate example app from the `aranya-cosmos` root and create two working directories for persistent state of each Aranya instance:
	```bash
	cd ./examples/rust/cosmos-gate
	mkdir gate-daemon
	mkdir flight-daemon
	```

1. Run the initializer:
	```bash
	cargo run --bin cosmos-gate-init <path_to_aranya-daemon_binary> <path_to_gate_daemon_dir> <path_to_flight_daemon_dir>
	```

	What this does:

	- Creates a team owned by the ground instance, adds the flight instance as a member
	- Persists onboarding state inside the `gate-daemon` and `flight-daemon` directories
	- Produces ready-to-ship working dirs that you can reuse on other machines to skip re-onboarding

3. Start the ground REST server and point it at the ground working directory:
	```bash
	cargo run --bin cosmos-gate-server  <path_to_aranya-daemon_binary> <path_to_gate_daemon_dir>
	```

If successful, the server listens on `127.0.0.1` using its default port. Use this URL as the `rest_endpoint` in your COSMOS dispatcher configuration.

### Running the Demo

You should have COSMOS and the ground REST server both still running in the background.

1. **Start the mock target container**

In another terminal, enter the subdirectory root for the `openc3-cosmos-gate` plugin.

Build and run the container from the subdirectory root:
```bash
cd tools
docker build -t target .
docker run --rm --name target -p 6200:6200/udp target:latest
```

- The Python app emits telemetry every second to the configured UDP port as defined in `openc3-cosmos-gate/targets/GATE/cmd_tlm/tlm.txt`.
- In the COSMOS CmdTlmServer view, observe `rx bytes` and `tlm pkts` increase every second.
- Use the Packet Viewer tool to inspect inbound telemetry.

2. **Test the integration**
   - Open the Command Sender in COSMOS.
   - Two telecommands are defined for the `GATE` target:
     - `NOOP`, the dispatcher skips the Aranya gate for this command, check CmdTlmServer logs.
     - `ARANYA_EP_EXP1`, CmdTlmServer logs should show the dispatcher posting the packet to the Aranya gate. The Aranya gate logs should show a received packet. If the policy allows, the gate returns a serialized command, the dispatcher inserts it into the `SER_CMD` field, then the packet is sent to the mock target, which logs receipt.

> The dispatcher behavior is keyed off the CCSDS function code field. Feel free to modify the dispatcher script to adapt behavior.


## Maintainers

The Aranya+COSMOS integration is led by [Matteo Calabrese](https://github.com/matcala).
The Aranya project, on which this project is based, is maintained by software engineers at
[SpiderOak](https://spideroak.com/).

