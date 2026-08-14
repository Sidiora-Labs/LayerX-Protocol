// SPDX-License-Identifier: LicenseRef-Centra-ai-Protocol
pragma solidity ^0.8.24;

import {Predeploys} from "../deployment/Predeploys.sol";
import {Preinstalls} from "../deployment/Preinstalls.sol";
import {SemverComp} from "../libraries/SemverComp.sol";
import {LayerXComponent} from "../security/LayerXComponent.sol";
import {
    ManagerUnauthorized,
    ManagerNotInitialized,
    ManagerAlreadyInitialized,
    InvalidManagerConfiguration,
    InvalidComponent,
    SelectorNotAllowed
} from "./BlockErrors.sol";

contract ManagerContainer is LayerXComponent {
    address public immutable governanceTimelock;
    uint192 public currentRelease;
    bytes32 public currentManifestRoot;
    address public migrator;
    bool public initialized;

    mapping(bytes32 => Preinstalls.ComponentManifest) private manifests;
    mapping(bytes32 => mapping(bytes4 => bool)) private selectorPermission;
    mapping(bytes32 => bytes4[]) private roleSelectors;

    event ManagerInitialized(bytes32 indexed manifestRoot, bytes32 indexed configHash, uint192 release);
    event MigratorSet(address indexed migrator);
    event SystemReleaseAdvanced(
        bytes32 indexed migrationId, uint192 sourceRelease, uint192 targetRelease, bytes32 configHash
    );

    constructor(address governance, bytes32 configHash, uint192 initialRelease)
        LayerXComponent(Predeploys.CONTRACTS_MANAGER, configHash, initialRelease)
    {
        if (governance == address(0) || configHash == bytes32(0) || initialRelease == 0) {
            revert InvalidManagerConfiguration();
        }
        governanceTimelock = governance;
        currentRelease = initialRelease;
    }

    modifier onlyGovernance() {
        if (msg.sender != governanceTimelock) {
            revert ManagerUnauthorized(msg.sender);
        }
        _;
    }

    function initialize(
        Preinstalls.ComponentManifest[] calldata initialManifests,
        bytes4[][] calldata selectorAllowlists
    ) external onlyGovernance {
        if (initialized) revert ManagerAlreadyInitialized();
        if (selectorAllowlists.length != Predeploys.COUNT) {
            revert InvalidManagerConfiguration();
        }
        bytes32 root = Preinstalls.validateComplete(initialManifests);
        for (uint256 i = 0; i < Predeploys.COUNT; ++i) {
            Preinstalls.ComponentManifest calldata manifest = initialManifests[i];
            if (manifest.configHash != staticConfigHash) {
                revert InvalidComponent(manifest.role, manifest.component);
            }
            manifests[manifest.role] = manifest;
            bytes4 previous;
            if (selectorAllowlists[i].length == 0) {
                revert SelectorNotAllowed(manifest.role, bytes4(0));
            }
            for (uint256 j = 0; j < selectorAllowlists[i].length; ++j) {
                bytes4 selector = selectorAllowlists[i][j];
                if (selector == bytes4(0) || (j != 0 && selector <= previous)) {
                    revert SelectorNotAllowed(manifest.role, selector);
                }
                selectorPermission[manifest.role][selector] = true;
                roleSelectors[manifest.role].push(selector);
                previous = selector;
            }
        }
        currentManifestRoot = root;
        initialized = true;
        emit ManagerInitialized(root, staticConfigHash, currentRelease);
    }

    function setMigrator(address migrationExecutor) external onlyGovernance {
        if (!initialized) revert ManagerNotInitialized();
        if (migrationExecutor == address(0) || migrationExecutor.code.length == 0) {
            revert InvalidManagerConfiguration();
        }
        migrator = migrationExecutor;
        emit MigratorSet(migrationExecutor);
    }

    function completeMigration(bytes32 migrationId, uint192 targetRelease, bytes32 configHash) external {
        if (!initialized) revert ManagerNotInitialized();
        if (msg.sender != migrator) revert ManagerUnauthorized(msg.sender);
        if (
            migrationId == bytes32(0) || configHash != staticConfigHash
                || !SemverComp.isStrictUpgrade(currentRelease, targetRelease)
        ) {
            revert InvalidManagerConfiguration();
        }
        uint192 source = currentRelease;
        currentRelease = targetRelease;
        emit SystemReleaseAdvanced(migrationId, source, targetRelease, configHash);
    }

    function componentForRole(bytes32 role) public view returns (address) {
        if (!initialized) revert ManagerNotInitialized();
        address component = manifests[role].component;
        if (component == address(0)) revert InvalidComponent(role, component);
        return component;
    }

    function manifestForRole(bytes32 role) external view returns (Preinstalls.ComponentManifest memory) {
        componentForRole(role);
        return manifests[role];
    }

    function manifestAt(uint256 index) external view returns (Preinstalls.ComponentManifest memory) {
        return manifests[Predeploys.roleAt(index)];
    }

    function isSelectorAllowed(bytes32 role, bytes4 selector) public view returns (bool) {
        if (!initialized) revert ManagerNotInitialized();
        return selectorPermission[role][selector];
    }

    function requireAllowed(bytes32 role, address target, bytes4 selector) external view {
        if (componentForRole(role) != target) {
            revert InvalidComponent(role, target);
        }
        if (!isSelectorAllowed(role, selector)) {
            revert SelectorNotAllowed(role, selector);
        }
    }

    function selectorsForRole(bytes32 role) external view returns (bytes4[] memory) {
        componentForRole(role);
        return roleSelectors[role];
    }

    function roleCount() external pure returns (uint256) {
        return Predeploys.COUNT;
    }
}
