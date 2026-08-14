// SPDX-License-Identifier: LicenseRef-Centra-ai-Protocol
pragma solidity ^0.8.24;

import {Constants} from "../libraries/Constants.sol";
import {UUPSNotUpgradeable} from "../security/UUPSNotUpgradeable.sol";
import {Predeploys} from "./Predeploys.sol";

contract Blueprint is UUPSNotUpgradeable {
    error ManagerOnly();
    error InvalidBlueprint();
    error DeploymentCollision(address predicted, bytes32 expectedCodeHash, bytes32 actualCodeHash);
    error DeploymentFailed(bytes32 role);
    error BlueprintSealed();

    address public immutable manager;
    bytes32 public immutable staticConfigHash;
    uint192 public immutable releaseVersion;
    mapping(bytes32 => address) public deploymentForRole;
    bool public deploymentsSealed;

    event ComponentDeployed(
        bytes32 indexed role, address indexed component, bytes32 initCodeHash, bytes32 runtimeCodeHash, bytes32 salt
    );
    event DeploymentsSealed(bytes32 indexed staticConfigHash, uint192 release);

    constructor(address deploymentManager, bytes32 configHash, uint192 release) {
        if (deploymentManager == address(0) || configHash == bytes32(0) || release == 0) revert InvalidBlueprint();
        manager = deploymentManager;
        staticConfigHash = configHash;
        releaseVersion = release;
    }

    function salt(bytes32 role, bytes32 initCodeHash) public view returns (bytes32) {
        if (role == bytes32(0) || initCodeHash == bytes32(0)) {
            revert InvalidBlueprint();
        }
        if (!Predeploys.isKnown(role)) revert InvalidBlueprint();
        return keccak256(
            abi.encode(
                Constants.DOMAIN_DEPLOYMENT,
                Constants.PROTOCOL_VERSION,
                block.chainid,
                address(this),
                staticConfigHash,
                releaseVersion,
                role,
                initCodeHash
            )
        );
    }

    function blueprintHash(bytes32 role, bytes32 initCodeHash, bytes32 expectedRuntimeCodeHash)
        external
        view
        returns (bytes32)
    {
        if (expectedRuntimeCodeHash == bytes32(0)) {
            revert InvalidBlueprint();
        }
        return keccak256(
            abi.encode(
                salt(role, initCodeHash), initCodeHash, expectedRuntimeCodeHash, staticConfigHash, releaseVersion
            )
        );
    }

    function predict(bytes32 role, bytes32 initCodeHash) public view returns (address) {
        bytes32 digest =
            keccak256(abi.encodePacked(bytes1(0xff), address(this), salt(role, initCodeHash), initCodeHash));
        return address(uint160(uint256(digest)));
    }

    function deploy(bytes32 role, bytes calldata initCode, bytes32 expectedRuntimeCodeHash)
        external
        returns (address component)
    {
        if (msg.sender != manager) revert ManagerOnly();
        if (deploymentsSealed) revert BlueprintSealed();
        if (initCode.length == 0 || expectedRuntimeCodeHash == bytes32(0)) {
            revert InvalidBlueprint();
        }
        bytes32 initCodeHash = keccak256(initCode);
        bytes32 deploymentSalt = salt(role, initCodeHash);
        address predicted = predict(role, initCodeHash);
        address existing = deploymentForRole[role];
        if (existing != address(0) && existing != predicted) {
            revert DeploymentCollision(existing, expectedRuntimeCodeHash, existing.codehash);
        }
        if (predicted.code.length != 0) {
            if (predicted.codehash != expectedRuntimeCodeHash) {
                revert DeploymentCollision(predicted, expectedRuntimeCodeHash, predicted.codehash);
            }
            deploymentForRole[role] = predicted;
            return predicted;
        }
        bytes memory creationCode = initCode;
        assembly ("memory-safe") {
            component := create2(0, add(creationCode, 32), mload(creationCode), deploymentSalt)
        }
        if (component == address(0) || component != predicted) {
            revert DeploymentFailed(role);
        }
        if (component.codehash != expectedRuntimeCodeHash) {
            revert DeploymentCollision(component, expectedRuntimeCodeHash, component.codehash);
        }
        deploymentForRole[role] = component;
        emit ComponentDeployed(role, component, initCodeHash, expectedRuntimeCodeHash, deploymentSalt);
    }

    function seal() external {
        if (msg.sender != manager) revert ManagerOnly();
        if (deploymentsSealed) revert BlueprintSealed();
        deploymentsSealed = true;
        emit DeploymentsSealed(staticConfigHash, releaseVersion);
    }
}
