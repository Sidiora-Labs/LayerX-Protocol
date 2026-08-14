// SPDX-License-Identifier: LicenseRef-Centra-ai-Protocol
pragma solidity ^0.8.24;

import {LayerXTimelock} from "../contracts/governance/LayerXTimelock.sol";

interface TimelockVm {
    function warp(uint256 timestamp) external;
    function expectRevert(bytes4 selector) external;
}

contract LayerXTimelockTest {
    TimelockVm private constant vm = TimelockVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    bytes32 private constant CONFIG = keccak256("timelock-test-config");
    uint192 private constant RELEASE = uint192(1) << 128;

    function testGovernanceCannotBypassDelay() public {
        LayerXTimelock timelock =
            new LayerXTimelock(2 days, 7 days, address(this), address(this), address(this), 1 ether, CONFIG, RELEASE);
        bytes memory data = abi.encodeCall(LayerXTimelock.updateMinDelay, (3 days));
        bytes32 salt = keccak256("delay-update");
        timelock.schedule(address(timelock), 0, data, salt, 2 days);
        vm.expectRevert(LayerXTimelock.OperationNotReady.selector);
        timelock.execute(address(timelock), 0, data, salt, 0);
        vm.warp(block.timestamp + 2 days);
        timelock.execute(address(timelock), 0, data, salt, 0);
        require(timelock.minDelay() == 3 days, "delay not enacted");
    }

    function testGuardianCancelsQueuedOperation() public {
        LayerXTimelock timelock =
            new LayerXTimelock(2 days, 7 days, address(this), address(this), address(this), 1 ether, CONFIG, RELEASE);
        bytes memory data = abi.encodeCall(LayerXTimelock.updateMinDelay, (3 days));
        bytes32 salt = keccak256("cancelled-update");
        bytes32 id = timelock.schedule(address(timelock), 0, data, salt, 2 days);
        timelock.cancel(id);
        vm.warp(block.timestamp + 2 days);
        vm.expectRevert(LayerXTimelock.OperationNotReady.selector);
        timelock.execute(address(timelock), 0, data, salt, 0);
    }

    function testRoleChangesRequireTheirOwnCompletedDelay() public {
        LayerXTimelock timelock =
            new LayerXTimelock(2 days, 7 days, address(this), address(this), address(this), 1 ether, CONFIG, RELEASE);
        address newProposer = address(0xA11CE);
        bytes memory data = abi.encodeCall(LayerXTimelock.setRole, (uint8(1), newProposer, true));
        bytes32 salt = keccak256("proposer-role");
        timelock.schedule(address(timelock), 0, data, salt, 2 days);
        require(!timelock.proposer(newProposer), "role changed before execution");
        vm.warp(block.timestamp + 2 days);
        timelock.execute(address(timelock), 0, data, salt, 0);
        require(timelock.proposer(newProposer), "delayed role change absent");
    }
}
