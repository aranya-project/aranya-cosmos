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

## 2025 Space Software Summit Workshop and Demo

Aranya COSMOS was introduced at the [2025 Space Software Summit Workshop](https://www.spacesoftwaresummit.com/), and a brief demo was shared with participants. 

If you're here because you attended our workshop, thank you and welcome!

You can run a demo of this integration by cloning this repository and following the step-by-step instructions in the [`cosmos-gate`](examples/rust/cosmos-gate) and [`openc3-cosmos-gate`](https://github.com/matcala/openc3-cosmos-gate) directories. 

## Supported Platforms and Status

Currently, Aranya COSMOS has been tested on Apple Silicon macOS but is expected to also work on Linux. To better understand how to tweak the demo for a Linux environment, refer to the instructions in the [`openc3-cosmos-gate`](https://github.com/matcala/openc3-cosmos-gate) submodule.

> Aranya COSMOS is in its early stages and is not yet ready for production use.

## Maintainers

The Aranya+COSMOS integration is led by [Matteo Calabrese](https://github.com/matcala).
The Aranya project, on which this project is based, is maintained by software engineers at
[SpiderOak](https://spideroak.com/).
