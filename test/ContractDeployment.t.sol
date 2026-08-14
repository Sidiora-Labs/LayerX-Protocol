// SPDX-License-Identifier: LicenseRef-Centra-ai-Protocol
pragma solidity ^0.8.24;

import {Features} from "../contracts/config/Features.sol";
import {StaticConfig} from "../contracts/config/StaticConfig.sol";
import {Blueprint} from "../contracts/deployment/Blueprint.sol";
import {Predeploys} from "../contracts/deployment/Predeploys.sol";
import {ILayerXComponent, Preinstalls} from "../contracts/deployment/Preinstalls.sol";
import {Constants} from "../contracts/libraries/Constants.sol";
import {SemverComp} from "../contracts/libraries/SemverComp.sol";

interface DeploymentVm {
    function etch(address account, bytes calldata code) external;
    function expectPartialRevert(bytes4 selector) external;
    function prank(address sender) external;
}

contract DeploymentHarness {
    function parseVersion(string calldata version) external pure returns (uint192) {
        return SemverComp.parseRelease(version);
    }

    function compareVersions(uint192 first, uint192 second) external pure returns (int8) {
        return SemverComp.compare(first, second);
    }

    function validateFeatures(uint256 features) external pure {
        Features.validate(features);
    }

    function hashAssets(StaticConfig.AssetDefinition[] memory assets) external pure returns (bytes32) {
        return StaticConfig.hashAssets(assets);
    }

    function configHash(StaticConfig.Config memory config, uint256 actualChainId) external pure returns (bytes32) {
        return StaticConfig.hash(config, actualChainId);
    }

    function roleAt(uint256 index) external pure returns (bytes32) {
        return Predeploys.roleAt(index);
    }

    function predeployCount() external pure returns (uint256) {
        return Predeploys.COUNT;
    }

    function componentInterfaceId() external pure returns (bytes4) {
        return Preinstalls.interfaceId();
    }

    function validateComponent(Preinstalls.ComponentManifest memory manifest) external view {
        Preinstalls.validate(manifest);
    }

    function validateManifest(Preinstalls.ComponentManifest[] memory manifests) external view returns (bytes32) {
        return Preinstalls.validateComplete(manifests);
    }
}

contract BlueprintTarget {
    uint256 public immutable value;

    constructor(uint256 initialValue) {
        value = initialValue;
    }
}

contract ComponentAttestation is ILayerXComponent {
    bytes32 public immutable override componentRole;
    bytes32 public immutable override staticConfigHash;
    uint192 public immutable override releaseVersion;
    uint16 public immutable override storageLayoutVersion;

    constructor(bytes32 role, bytes32 configHash, uint192 release, uint16 layout) {
        componentRole = role;
        staticConfigHash = configHash;
        releaseVersion = release;
        storageLayoutVersion = layout;
    }
}

    contract ContractDeploymentTest {
        DeploymentVm private constant vm = DeploymentVm(address(uint160(uint256(keccak256("hevm cheat code")))));
        DeploymentHarness private harness;
        uint192 private release;

        function setUp() public {
            harness = new DeploymentHarness();
            release = harness.parseVersion("1.0.0");
        }

        function testStrictStableSemverComparison() public view {
            uint192 v100 = harness.parseVersion("1.0.0");
            uint192 v101 = harness.parseVersion("1.0.1");
            uint192 v110 = harness.parseVersion("1.1.0");
            uint192 v200 = harness.parseVersion("2.0.0");
            require(harness.compareVersions(v100, v101) == -1, "patch");
            require(harness.compareVersions(v101, v110) == -1, "minor");
            require(harness.compareVersions(v110, v200) == -1, "major");
            require(harness.compareVersions(v200, v200) == 0, "equal");
        }

        function testSemverRejectsLeadingZeroAndPrerelease() public {
            vm.expectPartialRevert(SemverComp.InvalidSemanticVersion.selector);
            harness.parseVersion("01.0.0");
            vm.expectPartialRevert(SemverComp.InvalidSemanticVersion.selector);
            harness.parseVersion("1.0.0-alpha");
            vm.expectPartialRevert(SemverComp.InvalidSemanticVersion.selector);
            harness.parseVersion("1.0");
        }

        function testFeaturesRejectForeignAndUnknownFacilities() public {
            harness.validateFeatures(Features.ERC20_CUSTODY | Features.EMERGENCY_EXIT);
            vm.expectPartialRevert(Features.ForeignChainFacility.selector);
            harness.validateFeatures(Features.ARBITRUM_PRECOMPILES);
            vm.expectPartialRevert(Features.UnsupportedFeature.selector);
            harness.validateFeatures(1 << 100);
        }

        function testStaticConfigBindsAssetsChainAndAuthorities() public view {
            StaticConfig.AssetDefinition[] memory assets = _assets();
            bytes32 assetsRoot = harness.hashAssets(assets);
            StaticConfig.Config memory config = _config(assetsRoot);
            bytes32 first = harness.configHash(config, block.chainid);
            config.challengeWindow += 1;
            bytes32 second = harness.configHash(config, block.chainid);
            require(first != second, "config field not bound");
        }

        function testStaticConfigRejectsWrongChainAndUnorderedAssets() public {
            StaticConfig.AssetDefinition[] memory assets = _assets();
            bytes32 assetsRoot = harness.hashAssets(assets);
            StaticConfig.Config memory config = _config(assetsRoot);
            vm.expectPartialRevert(StaticConfig.StaticConfigWrongChain.selector);
            harness.configHash(config, block.chainid + 1);
            StaticConfig.AssetDefinition memory temporary = assets[0];
            assets[0] = assets[1];
            assets[1] = temporary;
            vm.expectPartialRevert(StaticConfig.AssetDefinitionsNotOrdered.selector);
            harness.hashAssets(assets);
        }

        function testBlueprintDeploysAtPredictedAddressAndIsIdempotent() public {
            Blueprint blueprint = new Blueprint(address(this), keccak256("config"), release);
            bytes32 role = Predeploys.VAULT;
            bytes memory initCode = abi.encodePacked(type(BlueprintTarget).creationCode, abi.encode(uint256(7)));
            BlueprintTarget runtimeReference = new BlueprintTarget(7);
            bytes32 expectedRuntime = address(runtimeReference).codehash;
            address predicted = blueprint.predict(role, keccak256(initCode));
            address deployed = blueprint.deploy(role, initCode, expectedRuntime);
            require(deployed == predicted && deployed.codehash == expectedRuntime, "deterministic deployment");
            require(BlueprintTarget(deployed).value() == 7, "constructor binding");
            require(blueprint.deploy(role, initCode, expectedRuntime) == deployed, "idempotence");
        }

        function testBlueprintRejectsOccupiedAndWrongRuntimeCode() public {
            Blueprint blueprint = new Blueprint(address(this), keccak256("config"), release);
            BlueprintTarget runtimeReference = new BlueprintTarget(9);
            bytes memory initCode = abi.encodePacked(type(BlueprintTarget).creationCode, abi.encode(uint256(9)));
            bytes32 initCodeHash = keccak256(initCode);
            address predicted = blueprint.predict(Predeploys.ASSET_REGISTRY, initCodeHash);
            vm.etch(predicted, hex"6000");
            vm.expectPartialRevert(Blueprint.DeploymentCollision.selector);
            blueprint.deploy(Predeploys.ASSET_REGISTRY, initCode, address(runtimeReference).codehash);
            vm.expectPartialRevert(Blueprint.DeploymentCollision.selector);
            blueprint.deploy(Predeploys.GUARANTOR_BOND, initCode, keccak256("wrong"));
        }

        function testBlueprintRejectsNonManager() public {
            Blueprint blueprint = new Blueprint(address(this), keccak256("config"), release);
            bytes memory initCode = abi.encodePacked(type(BlueprintTarget).creationCode, abi.encode(uint256(1)));
            BlueprintTarget runtimeReference = new BlueprintTarget(1);
            vm.expectPartialRevert(Blueprint.ManagerOnly.selector);
            vm.prank(address(0xBAD));
            blueprint.deploy(Predeploys.TIMELOCK, initCode, address(runtimeReference).codehash);
        }

        function testPreinstallManifestIsExhaustiveAndAttested() public {
            Preinstalls.ComponentManifest[] memory manifests = _manifest();
            bytes32 root = harness.validateManifest(manifests);
            require(root != bytes32(0), "manifest root");
            manifests[4].runtimeCodeHash = keccak256("wrong");
            vm.expectPartialRevert(Preinstalls.InvalidComponentManifest.selector);
            harness.validateManifest(manifests);
        }

        function testPreinstallManifestRejectsDuplicateAddress() public {
            Preinstalls.ComponentManifest[] memory manifests = _manifest();
            manifests[1].component = manifests[0].component;
            vm.expectPartialRevert(Preinstalls.DuplicateComponentAddress.selector);
            harness.validateManifest(manifests);
        }

        function testPredeployRolesAreExhaustiveAndUnique() public view {
            uint256 count = harness.predeployCount();
            require(count == 13, "role count");
            for (uint256 i = 0; i < count; ++i) {
                bytes32 role = harness.roleAt(i);
                require(role != bytes32(0), "empty role");
                for (uint256 j = 0; j < i; ++j) {
                    require(role != harness.roleAt(j), "duplicate role");
                }
            }
        }

        function _assets() private pure returns (StaticConfig.AssetDefinition[] memory assets) {
            assets = new StaticConfig.AssetDefinition[](2);
            assets[0] = StaticConfig.AssetDefinition({
                assetId: bytes32(uint256(1)),
                token: address(0x1001),
                tokenDecimals: 6,
                protocolDecimals: 18,
                minimumDeposit: 1_000_000,
                custodyCap: 1_000_000_000_000
            });
            assets[1] = StaticConfig.AssetDefinition({
                assetId: bytes32(uint256(2)),
                token: address(0x1002),
                tokenDecimals: 18,
                protocolDecimals: 18,
                minimumDeposit: 1 ether,
                custodyCap: 1_000_000 ether
            });
        }

        function _config(bytes32 assetRoot) private view returns (StaticConfig.Config memory) {
            return StaticConfig.Config({
                chainId: block.chainid,
                protocolVersion: Constants.PROTOCOL_VERSION,
                releaseVersion: release,
                governanceTimelock: address(0x1000),
                emergencyCouncil: address(0x2000),
                genesisStateRoot: keccak256("genesis"),
                challengeWindow: 7 days,
                checkpointLivenessBound: 1 days,
                enabledFeatures: Features.ERC20_CUSTODY | Features.CHECKPOINT_CHALLENGES | Features.WITHDRAWAL_CLAIMS
                    | Features.EMERGENCY_EXIT | Features.RESERVE_RECONCILIATION,
                assetDefinitionsRoot: assetRoot
            });
        }

        function _manifest() private returns (Preinstalls.ComponentManifest[] memory manifests) {
            manifests = new Preinstalls.ComponentManifest[](Predeploys.COUNT);
            bytes32 configHash = keccak256("manifest-config");
            bytes4 expectedInterface = harness.componentInterfaceId();
            for (uint256 i = 0; i < manifests.length; ++i) {
                bytes32 role = harness.roleAt(i);
                ComponentAttestation component =
                    new ComponentAttestation(role, configHash, release, Constants.STORAGE_LAYOUT_VERSION);
                manifests[i] = Preinstalls.ComponentManifest({
                    role: role,
                    component: address(component),
                    interfaceId: expectedInterface,
                    runtimeCodeHash: address(component).codehash,
                    configHash: configHash,
                    release: release,
                    storageLayout: Constants.STORAGE_LAYOUT_VERSION
                });
            }
        }
    }
