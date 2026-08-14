// SPDX-License-Identifier: LicenseRef-Centra-ai-Protocol
pragma solidity ^0.8.24;

abstract contract DelegateCallAware {
    error DirectCallOnly();
    error DelegateCallOnly();

    address private immutable implementationAddress = address(this);

    modifier onlyDirectCall() {
        if (address(this) != implementationAddress) revert DirectCallOnly();
        _;
    }

    modifier onlyDelegateCall() {
        if (address(this) == implementationAddress) revert DelegateCallOnly();
        _;
    }

    function isDelegateCall() public view returns (bool) {
        return address(this) != implementationAddress;
    }
}
