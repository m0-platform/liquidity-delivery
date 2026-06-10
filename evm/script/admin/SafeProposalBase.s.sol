// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { console2 } from "../../lib/forge-std/src/Script.sol";
import { Enum } from "../../lib/safe-utils/lib/safe-smart-account/contracts/common/Enum.sol";
import { Safe } from "../../lib/safe-utils/src/Safe.sol";

import { ScriptBase } from "../ScriptBase.s.sol";

/// @title SafeProposalBase
/// @notice Base contract for scripts that propose transactions to the Safe multisig
///         via the Safe Transaction Service (using m0-foundation/safe-utils).
/// @dev Required environment variables:
///        SAFE_ADDRESS            - The Safe multisig address
///        PROPOSER_PRIVATE_KEY    - Private key of a Safe owner/proposer (software signing), OR
///        PROPOSER_ADDRESS        - Proposer address, used with LEDGER_DERIVATION_PATH for Ledger signing
///      Optional environment variables:
///        LEDGER_DERIVATION_PATH  - Sign the proposal with a Ledger via `cast wallet sign --ledger`
///        SAFE_NONCE              - Explicit Safe nonce (to queue multiple proposals); defaults to the
///                                  current on-chain Safe nonce
///        DRY_RUN                 - If true, log the transaction without submitting the proposal
abstract contract SafeProposalBase is ScriptBase {
    Safe.Client internal _safeClient;

    /// @notice The Safe multisig address from the environment
    function _safe() internal view returns (address) {
        return vm.envAddress("SAFE_ADDRESS");
    }

    /// @notice The proposer address, derived from PROPOSER_PRIVATE_KEY if set, otherwise PROPOSER_ADDRESS
    function _proposer() internal returns (address) {
        uint256 proposerKey_ = vm.envOr("PROPOSER_PRIVATE_KEY", uint256(0));
        if (proposerKey_ != 0) return vm.rememberKey(proposerKey_);
        return vm.envAddress("PROPOSER_ADDRESS");
    }

    /// @notice Sign and propose a single transaction to the Safe Transaction Service
    /// @param target_ The contract the Safe should call
    /// @param data_ The calldata for the Safe transaction
    /// @return safeTxHash_ The hash of the proposed Safe transaction
    function _propose(address target_, bytes memory data_) internal returns (bytes32 safeTxHash_) {
        Safe.initialize(_safeClient, _safe());
        return _proposeTransaction(target_, data_, Enum.Operation.Call);
    }

    function _proposeTransaction(
        address target_,
        bytes memory data_,
        Enum.Operation operation_
    ) private returns (bytes32 safeTxHash_) {
        address proposer_ = _proposer();
        string memory derivationPath_ = vm.envOr("LEDGER_DERIVATION_PATH", string(""));

        uint256 nonce_ = vm.envOr("SAFE_NONCE", Safe.getNonce(_safeClient));

        console2.log("Safe:", _safe());
        console2.log("Proposer:", proposer_);
        console2.log("Target:", target_);
        console2.log("Operation:", operation_ == Enum.Operation.Call ? "Call" : "DelegateCall (batch)");
        console2.log("Nonce:", nonce_);
        console2.log("Calldata:");
        console2.logBytes(data_);

        if (vm.envOr("DRY_RUN", false)) {
            safeTxHash_ = Safe.getSafeTxHash(_safeClient, target_, 0, data_, operation_, nonce_);
            console2.log("");
            console2.log("DRY RUN - proposal NOT submitted to the Safe Transaction Service");
            console2.log("Safe transaction hash:");
            console2.logBytes32(safeTxHash_);
            return safeTxHash_;
        }

        safeTxHash_ = Safe.proposeTransaction(
            _safeClient,
            Safe.ExecTransactionParams({
                to: target_,
                value: 0,
                data: data_,
                operation: operation_,
                sender: proposer_,
                signature: Safe.sign(_safeClient, target_, data_, operation_, proposer_, nonce_, derivationPath_),
                nonce: nonce_
            })
        );

        console2.log("");
        console2.log("Proposed Safe transaction:");
        console2.logBytes32(safeTxHash_);
        console2.log("Review and confirm in the Safe UI: https://app.safe.global/transactions/queue");
    }
}
