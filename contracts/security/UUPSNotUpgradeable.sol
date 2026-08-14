// SPDX-License-Identifier: LicenseRef-Centra-ai-Protocol
pragma solidity ^0.8.24;

abstract contract UUPSNotUpgradeable {
    error UpgradesPermanentlyDisabled();

    function proxiableUUID() external pure returns (bytes32) {
        revert UpgradesPermanentlyDisabled();
    }

    function upgradeTo(address) external pure {
        revert UpgradesPermanentlyDisabled();
    }

    function upgradeToAndCall(address, bytes calldata) external payable {
        revert UpgradesPermanentlyDisabled();
    }
}
