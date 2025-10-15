# Aranya COSMOS

This repository holds the various crates and external resources that make up the Aranya COSMOS integration project, which adapts the [main Aranya
project](https://github.com/aranya-project/aranya) into a [COSMOS plugin](https://github.com/matcala/openc3-cosmos-gate).


## Supported Platforms and Status

Currently, Aranya COSMOS has been tested on Apple Silicon macOS but is expected to work also for Linux.

Aranya COSMOS is in its early stages and is not yet ready for production use.

## Resources

- [`cosmos-gate`](examples/rust/cosmos-gate) - the Aranya ground gate app crate.
- [`openc3-cosmos-gate`](https://github.com/matcala/openc3-cosmos-gate) - the OpenC3 COMSOS plugin to insert Aranya in the COSMOS command flow.


## Setup

Generally, the Aranya project requires a [rust development environment](https://www.rust-lang.org/).
Specific instructions for running a complete OpenC3 COSMOS + Aranya demo can be found in the resources referenced above.


## Maintainers

The Aranya+COSMOS integration is lead by [Matteo Calabrese](https://github.com/matcala).
The Aranya project, on which this project is based, is maintained by software engineers employed at
[SpiderOak](https://spideroak.com/).
