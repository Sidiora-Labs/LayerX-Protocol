// SPDX-License-Identifier: LicenseRef-Centra-ai-Protocol
pragma solidity ^0.8.24;

import {ILayerXComponent, Preinstalls} from "../contracts/deployment/Preinstalls.sol";
import {Predeploys} from "../contracts/deployment/Predeploys.sol";
import {Constants} from "../contracts/libraries/Constants.sol";
import {SemverComp} from "../contracts/libraries/SemverComp.sol";
import {ManagerContainer} from "../contracts/manager/ManagerContainer.sol";
import {ManagerMigrator} from "../contracts/manager/ManagerMigrator.sol";
import {StandardValidatorUtils} from "../contracts/manager/StandardValidatorUtils.sol";
import {
    ManagerUnauthorized,
    InvalidMigrationState,
    InvalidMigrationWindow,
    InvalidMigrationVersion,
    MigrationCommitmentMismatch,
    InvalidMigrationOperation,
    MigrationCodeHashMismatch,
    MigrationStorageLayoutMismatch,
    MigrationCallFailed,
    SelectorNotAllowed,
    InvalidComponent
} from "../contracts/manager/BlockErrors.sol";

interface ManagerVm {
    function deal(address account, uint256 balance) external;
    function expectPartialRevert(bytes4 selector) external;
    function prank(address sender) external;
    function warp(uint256 timestamp) external;
}

contract MigrationComponent is ILayerXComponent {
    error ComponentUnauthorized();
    error RequestedMigrationFailure();
    error InvalidLayoutTransition();

    bytes32 public immutable override componentRole;
    bytes32 public immutable override staticConfigHash;
    uint192 public immutable override releaseVersion;
    address public immutable governance;
    uint16 public override storageLayoutVersion;
    address public migrator;
    uint256 public migrationCount;
    uint256 public receivedValue;

    constructor(bytes32 role, bytes32 configHash, uint192 release, address governanceAuthority) {
        componentRole = role;
        staticConfigHash = configHash;
        releaseVersion = release;
        governance = governanceAuthority;
        storageLayoutVersion = Constants.STORAGE_LAYOUT_VERSION;
    }

    function authorizeMigrator(address migrationExecutor) external {
        if (msg.sender != governance || migrator != address(0) || migrationExecutor.code.length == 0) {
            revert ComponentUnauthorized();
        }
        migrator = migrationExecutor;
    }

    function migrate(uint16 nextLayout, bool fail) external payable {
        if (msg.sender != migrator) revert ComponentUnauthorized();
        if (fail) revert RequestedMigrationFailure();
        if (nextLayout < storageLayoutVersion || nextLayout > storageLayoutVersion + 1) {
            revert InvalidLayoutTransition();
        }
        storageLayoutVersion = nextLayout;
        ++migrationCount;
        receivedValue += msg.value;
    }
}

    contract ContractManagerTest {
        ManagerVm private constant vm = ManagerVm(address(uint160(uint256(keccak256("hevm cheat code")))));

        bytes32 private constant CONFIG_HASH = keccak256("manager-config");
        ManagerContainer private container;
        ManagerMigrator private migrator;
        MigrationComponent[] private components;
        uint192 private release100;
        uint192 private release101;
        uint192 private release110;

        function setUp() public {
            release100 = SemverComp.parseRelease("1.0.0");
            release101 = SemverComp.parseRelease("1.0.1");
            release110 = SemverComp.parseRelease("1.1.0");
            container = new ManagerContainer(address(this), CONFIG_HASH, release100);

            Preinstalls.ComponentManifest[] memory manifests = new Preinstalls.ComponentManifest[](Predeploys.COUNT);
            bytes4[][] memory allowlists = new bytes4[][](Predeploys.COUNT);
            for (uint256 i = 0; i < Predeploys.COUNT; ++i) {
                bytes32 role = Predeploys.roleAt(i);
                MigrationComponent component = new MigrationComponent(role, CONFIG_HASH, release100, address(this));
                components.push(component);
                manifests[i] = Preinstalls.ComponentManifest({
                    role: role,
                    component: address(component),
                    interfaceId: Preinstalls.interfaceId(),
                    runtimeCodeHash: address(component).codehash,
                    configHash: CONFIG_HASH,
                    release: release100,
                    storageLayout: Constants.STORAGE_LAYOUT_VERSION
                });
                allowlists[i] = new bytes4[](1);
                allowlists[i][0] = MigrationComponent.migrate.selector;
            }
            container.initialize(manifests, allowlists);
            migrator = new ManagerMigrator(container, address(this), address(this), 1 days, 7 days, 1_000_000, 1 ether);
            container.setMigrator(address(migrator));
            for (uint256 i = 0; i < components.length; ++i) {
                components[i].authorizeMigrator(address(migrator));
            }
        }

        function testExecutesCommittedMigrationExactlyOnce() public {
            StandardValidatorUtils.Operation[] memory operations = _operations(1, false);
            bytes32 migrationId = _stage(release101, operations, 1 days, 2 days);
            vm.warp(block.timestamp + 1 days);
            migrator.executeMigration(migrationId, operations);
            require(components[0].storageLayoutVersion() == 2, "layout");
            require(components[0].migrationCount() == 1, "count");
            require(container.currentRelease() == release101, "release");
            require(migrator.activeMigration() == bytes32(0), "active");
            vm.expectPartialRevert(InvalidMigrationState.selector);
            migrator.executeMigration(migrationId, operations);
        }

        function testMigrationCannotExecuteEarlyOrAfterExpiry() public {
            StandardValidatorUtils.Operation[] memory operations = _operations(1, false);
            bytes32 migrationId = _stage(release101, operations, 1 days, 2 days);
            vm.expectPartialRevert(InvalidMigrationWindow.selector);
            migrator.executeMigration(migrationId, operations);
            vm.warp(block.timestamp + 3 days + 1);
            vm.expectPartialRevert(InvalidMigrationWindow.selector);
            migrator.executeMigration(migrationId, operations);
        }

        function testRejectsVersionDowngradeAndConcurrentPlan() public {
            StandardValidatorUtils.Operation[] memory operations = _operations(1, false);
            vm.expectPartialRevert(InvalidMigrationVersion.selector);
            _stage(release100, operations, 1 days, 2 days);
            bytes32 migrationId = _stage(release101, operations, 1 days, 2 days);
            require(migrationId != bytes32(0), "stage");
            vm.expectPartialRevert(InvalidMigrationState.selector);
            _stage(release110, operations, 1 days, 2 days);
        }

        function testOrderedOperationRootRejectsReordering() public {
            StandardValidatorUtils.Operation[] memory operations = _operations(2, false);
            bytes32 migrationId = _stage(release101, operations, 1 days, 2 days);
            StandardValidatorUtils.Operation memory temporary = operations[0];
            operations[0] = operations[1];
            operations[1] = temporary;
            vm.warp(block.timestamp + 1 days);
            vm.expectPartialRevert(MigrationCommitmentMismatch.selector);
            migrator.executeMigration(migrationId, operations);
            require(
                components[0].migrationCount() == 0 && components[1].migrationCount() == 0, "mutation before root check"
            );
        }

        function testRejectsWrongTargetAndSelector() public {
            StandardValidatorUtils.Operation[] memory operations = _operations(1, false);
            operations[0].target = address(components[1]);
            operations[0].expectedCodeHashBefore = address(components[1]).codehash;
            operations[0].expectedCodeHashAfter = address(components[1]).codehash;
            vm.expectPartialRevert(InvalidComponent.selector);
            _stage(release101, operations, 1 days, 2 days);

            operations = _operations(1, false);
            operations[0].data = abi.encodeCall(MigrationComponent.authorizeMigrator, (address(migrator)));
            vm.expectPartialRevert(SelectorNotAllowed.selector);
            _stage(release101, operations, 1 days, 2 days);
        }

        function testRejectsValueCodeHashAndStorageViolations() public {
            StandardValidatorUtils.Operation[] memory operations = _operations(1, false);
            operations[0].value = 1 ether + 1;
            vm.expectPartialRevert(InvalidMigrationOperation.selector);
            _stage(release101, operations, 1 days, 2 days);

            operations = _operations(1, false);
            operations[0].expectedCodeHashBefore = keccak256("wrong");
            vm.expectPartialRevert(MigrationCodeHashMismatch.selector);
            _stage(release101, operations, 1 days, 2 days);

            operations = _operations(1, false);
            operations[0].expectedStorageLayoutBefore = 2;
            operations[0].expectedStorageLayoutAfter = 2;
            vm.expectPartialRevert(MigrationStorageLayoutMismatch.selector);
            _stage(release101, operations, 1 days, 2 days);
        }

        function testLaterFailureRollsBackEveryEarlierOperation() public {
            StandardValidatorUtils.Operation[] memory operations = _operations(2, false);
            operations[1].data = abi.encodeCall(MigrationComponent.migrate, (uint16(2), true));
            bytes32 migrationId = _stage(release101, operations, 1 days, 2 days);
            vm.warp(block.timestamp + 1 days);
            vm.expectPartialRevert(MigrationCallFailed.selector);
            migrator.executeMigration(migrationId, operations);
            require(
                components[0].storageLayoutVersion() == 1 && components[0].migrationCount() == 0,
                "partial first mutation"
            );
            require(
                components[1].storageLayoutVersion() == 1 && components[1].migrationCount() == 0,
                "failed target mutation"
            );
            require(migrator.activeMigration() == migrationId, "plan state did not roll back");
        }

        function testPostCallHashMismatchRollsBackTarget() public {
            StandardValidatorUtils.Operation[] memory operations = _operations(1, false);
            operations[0].expectedCodeHashAfter = keccak256("wrong-after");
            bytes32 migrationId = _stage(release101, operations, 1 days, 2 days);
            vm.warp(block.timestamp + 1 days);
            vm.expectPartialRevert(MigrationCodeHashMismatch.selector);
            migrator.executeMigration(migrationId, operations);
            require(
                components[0].storageLayoutVersion() == 1 && components[0].migrationCount() == 0, "post-check partial"
            );
        }

        function testPostCallLayoutMismatchRollsBackTarget() public {
            StandardValidatorUtils.Operation[] memory operations = _operations(1, false);
            operations[0].data = abi.encodeCall(MigrationComponent.migrate, (uint16(1), false));
            bytes32 migrationId = _stage(release101, operations, 1 days, 2 days);
            vm.warp(block.timestamp + 1 days);
            vm.expectPartialRevert(MigrationStorageLayoutMismatch.selector);
            migrator.executeMigration(migrationId, operations);
            require(
                components[0].storageLayoutVersion() == 1 && components[0].migrationCount() == 0,
                "layout post-check partial"
            );
        }

        function testCancellationAndAuthorityAreFailClosed() public {
            StandardValidatorUtils.Operation[] memory operations = _operations(1, false);
            vm.expectPartialRevert(ManagerUnauthorized.selector);
            vm.prank(address(0xBAD));
            migrator.stageMigration(release101, CONFIG_HASH, operations, 1 days, 2 days);
            bytes32 migrationId = _stage(release101, operations, 1 days, 2 days);
            migrator.cancelMigration(migrationId);
            vm.warp(block.timestamp + 1 days);
            vm.expectPartialRevert(InvalidMigrationState.selector);
            migrator.executeMigration(migrationId, operations);
        }

        function testContainerManifestAndSelectorInventoryIsComplete() public view {
            require(container.roleCount() == Predeploys.COUNT, "role count");
            require(container.currentManifestRoot() != bytes32(0), "root");
            for (uint256 i = 0; i < Predeploys.COUNT; ++i) {
                bytes32 role = Predeploys.roleAt(i);
                require(container.componentForRole(role) == address(components[i]), "component");
                bytes4[] memory selectors = container.selectorsForRole(role);
                require(
                    selectors.length == 1 && selectors[0] == MigrationComponent.migrate.selector, "selector inventory"
                );
            }
        }

        function _stage(
            uint192 targetRelease,
            StandardValidatorUtils.Operation[] memory operations,
            uint64 delay,
            uint64 validity
        ) private returns (bytes32) {
            return migrator.stageMigration(targetRelease, CONFIG_HASH, operations, delay, validity);
        }

        function _operations(uint256 count, bool failLast)
            private
            view
            returns (StandardValidatorUtils.Operation[] memory operations)
        {
            operations = new StandardValidatorUtils.Operation[](count);
            for (uint256 i = 0; i < count; ++i) {
                MigrationComponent component = components[i];
                uint16 beforeLayout = component.storageLayoutVersion();
                uint16 afterLayout = beforeLayout + 1;
                operations[i] = StandardValidatorUtils.Operation({
                    role: Predeploys.roleAt(i),
                    target: address(component),
                    value: 0,
                    gasLimit: 300_000,
                    data: abi.encodeCall(MigrationComponent.migrate, (afterLayout, failLast && i + 1 == count)),
                    expectedCodeHashBefore: address(component).codehash,
                    expectedCodeHashAfter: address(component).codehash,
                    expectedStorageLayoutBefore: beforeLayout,
                    expectedStorageLayoutAfter: afterLayout
                });
            }
        }
    }
