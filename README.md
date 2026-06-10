# M0 Liquidity Delivery

This repository contains the onchain, limit-order, settlement contracts for the M0 Liquidity Delivery Network.

The protocol is implemented for both EVM and SVM blockchains, in the respective `evm` and `svm` folders. READMEs within these folders provide more information about using and contributing to the respective codebases.

## Limit Order Protocol

The core component of the liquidity delivery system is the onchain Limit Order Protocol. This allows users to submit samechain or crosschain limit orders to exchange one token for another. The primary use case is stablecoin orchestration, but the protocol is agnostic to the asset used (minus lacking support for some exotic token types). While there are many implementations of this type of "intent-based" system, we have specifically built this version to have the following features:

- An Order is defined as an offer to exchange an amount of one token for a fixed amount of another token until a specified deadline.
- Orders can be partially filled to allow solvers to cycle their inventory to fill larger orders.
- Orders must be created on the chain where the source token resides, but the output of the order can be delivered to any configured destination.
- Orders may be cancelled and expired orders refunded on the destination chain. This sends a crosschain message back to the origin chain to issue a refund to the sender.
- The system does not place any trust assumptions on users or solvers. The only trusted parties are the bridge contracts and the messaging protocols that pass messages between chains.
- Users may specify an exclusive solver for their order (e.g. if they require a known counterparty). If no solver is specified, any solver can fill the order.
