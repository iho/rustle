# Rustle

<img src="./logo.png" alt="Rustle" width="500"/>

[![CI Status](https://img.shields.io/github/actions/workflow/status/blocksecteam/rustle/ci.yml?branch=main&label=ci)](https://github.com/blocksecteam/rustle/actions/workflows/ci.yml)
[![Build-Image Status](https://img.shields.io/github/actions/workflow/status/blocksecteam/rustle/build-image.yml?branch=main&label=build-image)](https://github.com/blocksecteam/rustle/actions/workflows/build-image.yml)
[![License: AGPL v3](https://img.shields.io/github/license/blocksecteam/rustle)](LICENSE)
[![AwesomeNEAR](https://img.shields.io/badge/Project-AwesomeNEAR-054db4)](https://awesomenear.com/rustle)
[![Devpost](https://img.shields.io/badge/Honorable%20Mention-Devpost-003e54)](https://devpost.com/software/rustle)

Rustle is an automatic static analyzer for NEAR smart contracts in Rust. It can help to locate tens of different vulnerabilities in NEAR smart contracts.
According to [DefiLlama](https://defillama.com/chain/Near), among the top 10 DApps in NEAR, 8 are audited by BlockSec. With rich audit experience and a deep understanding of NEAR protocol, we build this tool and share it with the community.

## Get started

### Prerequisite

#### Linux setup

Install the required toolkits with the following commands for **Rustle** in Linux. Commands are tested in Ubuntu 22.04 LTS.

```bash
# install Rust Toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# install LLVM 21
wget -O - https://apt.llvm.org/llvm-snapshot.gpg.key | sudo tee /etc/apt/keyrings/llvm.asc
echo "deb [signed-by=/etc/apt/keyrings/llvm.asc] http://apt.llvm.org/jammy/ llvm-toolchain-jammy-21 main" \
    | sudo tee /etc/apt/sources.list.d/llvm.list
sudo apt-get update
sudo apt-get install -y llvm-21 clang-21

# install Python toolchain
sudo apt install python3 python3-pip    # requires python >= 3.8
pip3 install -r utils/requirements.txt  # you need to clone this repo first

# add WASM target
rustup target add wasm32-unknown-unknown

# install other components
sudo apt install figlet
cargo install rustfilt

# [optional] useful tools for developing
LLVM_VERSION=21
sudo apt install clangd-$LLVM_VERSION clang-format-$LLVM_VERSION clang-tidy-$LLVM_VERSION
```

#### macOS setup

The following commands are for users using macOS, tested on Apple Silicon.

```bash
# install Rust Toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# install LLVM (latest via Homebrew)
brew install llvm

# install Python packages
pip3 install -r utils/requirements.txt  # you need to clone this repo first

# add WASM target
rustup target add wasm32-unknown-unknown

# install other components
brew install figlet coreutils gsed
cargo install rustfilt
```

#### Docker

We provide a Docker solution.

```bash
# build the image
docker build --build-arg UID=`id -u` --build-arg GID=`id -g` -t rustle .

# run a container from the image
docker run --name rustle -it -v `pwd`:/rustle -w /rustle rustle bash

# exec the container
docker start rustle
docker exec -it -w /rustle rustle bash
```

### Usage

```bash
./rustle [-t|--tg_dir <tg_dir>] [-d|--detector <detector_list>] [-o|--output <output_dir>] [-f|--format <format>] [-h|--help] <src_dir>
```

* `src_dir`: Path to the contract source.
* `tg_dir`: Path to the contract build target. Defaults to `src_dir`.
* `detector`: Detector list — comma-separated *detector IDs* or *group names*. Defaults to `all`.
    * `all` — enable every detector.
    * `high`, `medium`, `low`, `info` — enable detectors by severity (see [Detectors](#detectors)).
    * `nep-ft`, `nep-storage`, `nep-nft`, `nep-mt` — enable NEP-specific detector groups (see [NEP detector groups](#nep-detector-groups)).
    * Individual detector IDs from the [table below](#detectors).
* `output`: Directory where audit reports are written. Defaults to `./audit-result`.
* `format`: Output format — `csv` (default) or `sarif` (SARIF 2.1.0 for GitHub Code Scanning and CI).

> **Note:** if cargo places the compiled `.bc` bitcode outside `src_dir` (e.g. in a workspace root), use `-t|--tg_dir` to point Rustle at that directory.

#### Examples

```bash
# clone LiNEAR and run all detectors
git clone https://github.com/linear-protocol/LiNEAR.git ~/near-repo/LiNEAR
./rustle -t ~/near-repo/LiNEAR ~/near-repo/LiNEAR/contracts/linear

# run only high/medium severity detectors and save report to ~/linear-report
./rustle -t ~/near-repo/LiNEAR ~/near-repo/LiNEAR/contracts/linear \
    -d high,medium,complex-loop -o ~/linear-report

# output SARIF for GitHub Code Scanning
./rustle ~/my-contract --format sarif

# run bundled sample contracts
./rustle contracts/amm -d all
./rustle contracts/nft-marketplace -d all
./rustle contracts/staking -d all
```

A CSV report (`summary.csv`) is written to the output directory. When `--format sarif` is used, a `results.sarif` file is written instead.

### Suppressing findings

Create a `.rustle.toml` file in the project root to suppress specific findings:

```toml
[suppress]
# suppress a detector globally
detectors = ["timestamp"]

# suppress a detector for a specific file and/or function
[[suppress.rule]]
detector = "unsafe-math"
path = "src/math_helpers.rs"
function = "calculate_fee"
```

## Detectors

All vulnerabilities **Rustle** can find.

| Detector ID                | Description                                                                                  | Severity |
| -------------------------- | -------------------------------------------------------------------------------------------- | -------- |
| `unhandled-promise`        | [find `Promises` that are not handled](docs/detectors/unhandled-promise.md)                  | High     |
| `non-private-callback`     | [missing macro `#[private]` for callback functions](docs/detectors/non-private-callback.md)  | High     |
| `reentrancy`               | [find functions that are vulnerable to reentrancy attack](docs/detectors/reentrancy.md)      | High     |
| `unsafe-math`              | [lack of overflow check for arithmetic operation](docs/detectors/unsafe-math.md)             | High     |
| `self-transfer`            | [missing check of `sender != receiver`](docs/detectors/self-transfer.md)                     | High     |
| `incorrect-json-type`      | [incorrect type used in parameters or return values](docs/detectors/incorrect-json-type.md)  | High     |
| `unsaved-changes`          | [changes to collections are not saved](docs/detectors/unsaved-changes.md)                    | High     |
| `nft-approval-check`       | [find `nft_transfer` without check of `approval id`](docs/detectors/nft-approval-check.md)   | High     |
| `nft-owner-check`          | [find approve or revoke functions without owner check](docs/detectors/nft-owner-check.md)    | High     |
| `state-change-before-call` | [state mutation before a cross-contract call without rollback](docs/detectors/state-change-before-call.md) | High |
| `unchecked-promise-result` | [callback reads promise result without checking success](docs/detectors/unchecked-promise-result.md) | High |
| `missing-owner-check`      | [privileged function missing caller ownership verification](docs/detectors/missing-owner-check.md) | High |
| `promise-chain`            | [hardcoded literal index passed to `env::promise_result()`](docs/detectors/promise-chain.md) | High     |
| `div-before-mul`           | [precision loss due to incorrect operation order](docs/detectors/div-before-mul.md)          | Medium   |
| `round`                    | [rounding without specifying ceil or floor](docs/detectors/round.md)                         | Medium   |
| `lock-callback`            | [panic in callback function may lock contract](docs/detectors/lock-callback.md)              | Medium   |
| `yocto-attach`             | [no `assert_one_yocto` in privileged function](docs/detectors/yocto-attach.md)               | Medium   |
| `dup-collection-id`        | [duplicate id uses in collections](docs/detectors/dup-collection-id.md)                      | Medium   |
| `unregistered-receiver`    | [no panic on unregistered transfer receivers](docs/detectors/unregistered-receiver.md)       | Medium   |
| `nep${id}-interface`       | [find all unimplemented NEP interface functions](docs/detectors/nep-interface.md)             | Medium   |
| `prepaid-gas`              | [missing check of prepaid gas in callback](docs/detectors/prepaid-gas.md)                    | Low      |
| `non-callback-private`     | [macro `#[private]` used in non-callback function](docs/detectors/non-callback-private.md)   | Low      |
| `unused-ret`               | [function result not used or checked](docs/detectors/unused-ret.md)                          | Low      |
| `upgrade-func`             | [no upgrade function in contract](docs/detectors/upgrade-func.md)                            | Low      |
| `tautology`                | [tautology used in conditional branch](docs/detectors/tautology.md)                          | Low      |
| `storage-gas`              | [missing balance check for storage expansion](docs/detectors/storage-gas.md)                 | Low      |
| `unclaimed-storage-fee`    | [missing balance check before storage unregister](docs/detectors/unclaimed-storage-fee.md)   | Low      |
| `inconsistency`            | [use of similar but slightly different symbol](docs/detectors/inconsistency.md)              | Info     |
| `timestamp`                | [find all uses of `block_timestamp`](docs/detectors/timestamp.md)                            | Info     |
| `complex-loop`             | [find loops with complex logic that may cause DoS](docs/detectors/complex-loop.md)           | Info     |
| `ext-call`                 | [find all cross-contract invocations](docs/detectors/ext-call.md)                            | Info     |
| `promise-result`           | [find all uses of promise result](docs/detectors/promise-result.md)                          | Info     |
| `transfer`                 | [find all transfer actions](docs/detectors/transfer.md)                                      | Info     |
| `public-interface`         | [find all public interfaces](docs/detectors/public-interface.md)                             | Info     |

### NEP detector groups

Apart from the groups by severity level, **Rustle** provides detector groups for specific NEP standards.

[nep141]: https://github.com/near/NEPs/blob/master/neps/nep-0141.md
[nep145]: https://github.com/near/NEPs/blob/master/neps/nep-0145.md
[nep148]: https://github.com/near/NEPs/blob/master/neps/nep-0148.md
[nep171]: https://github.com/near/NEPs/blob/master/neps/nep-0171.md
[nep177]: https://github.com/near/NEPs/blob/master/neps/nep-0177.md
[nep178]: https://github.com/near/NEPs/blob/master/neps/nep-0178.md
[nep181]: https://github.com/near/NEPs/blob/master/neps/nep-0181.md
[nep199]: https://github.com/near/NEPs/blob/master/neps/nep-0199.md
[nep245]: https://github.com/near/NEPs/blob/master/neps/nep-0245.md
[nep246]: https://github.com/near/NEPs/blob/master/neps/nep-0246.md
[nep330]: https://github.com/near/NEPs/blob/master/neps/nep-0330.md

| NEP(s) | Group ID | Included detectors |
| ------ | -------- | ------------------ |
| [NEP-141][nep141], [NEP-148][nep148] | `nep-ft` | `nep141-interface`, `nep148-interface`, `self-transfer`, `unregistered-receiver` |
| [NEP-145][nep145] | `nep-storage` | `nep145-interface`, `unclaimed-storage-fee` |
| [NEP-171][nep171], [NEP-177][nep177], [NEP-178][nep178], [NEP-181][nep181], [NEP-199][nep199] | `nep-nft` | `nep171-interface`, `nep177-interface`, `nep178-interface`, `nep181-interface`, `nep199-interface`, `nft-approval-check`, `nft-owner-check` |
| [NEP-245][nep245], [NEP-246][nep246] | `nep-mt` | `nep245-interface`, `nep246-interface` |

Individual NEP interface checks (e.g. `nep141-interface`, `nep199-interface`, `nep330-interface`) can also be run directly. The `nep-interface` group runs all supported NEP interface checks at once.

**Supported NEP interface IDs:** 141, 145, 148, 171, 177, 178, 181, 199, 245, 246, 330.

## Sample contracts

The `contracts/` directory contains realistic NEAR contracts demonstrating common vulnerability patterns, useful for testing Rustle or learning about NEAR security:

| Contract | Vulnerability patterns |
| -------- | ---------------------- |
| `contracts/amm/` | `div-before-mul`, `timestamp`, `reentrancy`, `unchecked-promise-result`, `promise-chain` |
| `contracts/nft-marketplace/` | `missing-owner-check`, `nft-approval-check`, `yocto-attach`, `unchecked-promise-result` |
| `contracts/staking/` | `state-change-before-call`, `storage-gas`, `round`, `prepaid-gas` |

## Add new detectors

1. Fork this repo to your account.
2. Put the new detector under [/detectors](/detectors/).
3. Add a detection target in [/Makefile](/Makefile) with commands to run your detector.
4. Add the target to the `analysis` target dependency, its name to the [detector list](/rustle#L159) and the appropriate [severity group](/rustle#L192) in the `./rustle` script.
5. Add processing code in [utils/audit.py](/utils/audit.py) (refer to other detectors' code in `audit.py`).
6. Submit a pull request from your branch to main.

## Note

**Rustle** can be used in the development process to scan NEAR smart contracts iteratively. This can save a lot of manual effort and mitigate part of potential issues. However, vulnerabilities in complex logic or related to semantics are still the limitation of **Rustle**. Locating complicated semantic issues requires the experts in [BlockSec](https://blocksec.com/) to conduct exhaustive and thorough reviews. [Contact us](audit@blocksec.com) for audit service.

## License

This project is under the AGPLv3 License. See the [LICENSE](LICENSE) file for the full license text.
