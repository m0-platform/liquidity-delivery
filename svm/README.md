# M0 Limit Order Protocol (SVM)

This folder contains the Solana Virtual Machine (SVM) implementation of the M0 Limit Order Protocol. It uses the Anchor framework and CLI tools for compiling and testing the programs. The primary program is `programs/order_book`. It depends on M0's Portal V2 architecture for sending and receiving crosschain messages related to order fills.

## Development

### Installation

You need to install the following tools to use this repository. You can see the different install options in the [Anchor documentation](https://www.anchor-lang.com/docs/installation):

- **Solana CLI v2.1+** - The CLI comes with the local validator from Agave. It is recommended to use `agave-install` to install and manage Solana versions.
- **Anchor CLI v0.31.1** - It is recommended to use Anchor Version Manager (`avm`) to install Anchor to make it easy to switch between versions.

Ensure you have the right versions installed + activated by running:

```bash
solana --version
anchor --version
```

### Build

The programs in this folder can be built with Anchor.

```bash
anchor build
```

### Test

Tests for the `order_book` program are written in Rust using the [`anchor-litesvm`](https://github.com/brimigs/anchor-litesvm) framework. You can run the test suite with:

```bash
anchor test
```
