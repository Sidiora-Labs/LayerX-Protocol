// SPDX-License-Identifier: LicenseRef-Centra-ai-Protocol
pragma solidity ^0.8.24;

import {ILayerXAssetRegistry} from "../interfaces/ILayerXAssetRegistry.sol";
import {SafeTransfer} from "../libraries/SafeTransfer.sol";
import {Arithmetic} from "../libraries/Arithmetic.sol";
import {Governed} from "../security/Governed.sol";
import {ReentrancyLock} from "../security/ReentrancyLock.sol";
import {LayerXComponent} from "../security/LayerXComponent.sol";
import {Predeploys} from "../deployment/Predeploys.sol";

contract LayerXVault is Governed, ReentrancyLock, LayerXComponent {
    error InvalidDeposit();
    error InvalidRelease();
    error SettlementModuleOnly();

    ILayerXAssetRegistry public immutable assetRegistry;
    mapping(address => bool) public settlementModule;
    mapping(bytes32 => uint256) public totalCustodied;
    mapping(bytes32 => uint256) public totalReleased;
    mapping(address => mapping(bytes32 => uint64)) public depositNonce;
    mapping(bytes32 => bool) public recordedDeposit;
    mapping(bytes32 => bool) public releasedClaim;

    event CustodyDeposit(
        bytes32 indexed depositId,
        bytes32 indexed assetId,
        address indexed payer,
        bytes32 beneficiary,
        uint256 amount,
        uint64 nonce
    );
    event CustodyRelease(
        bytes32 indexed claimId,
        bytes32 indexed assetId,
        address indexed recipient,
        uint256 amount,
        address settlementModule
    );
    event SettlementModuleSet(address indexed module, bool enabled);

    constructor(
        ILayerXAssetRegistry registry,
        address governanceTimelock,
        address emergencyAuthority,
        bytes32 componentConfigHash,
        uint192 componentRelease
    )
        Governed(governanceTimelock, emergencyAuthority)
        LayerXComponent(Predeploys.VAULT, componentConfigHash, componentRelease)
    {
        if (address(registry) == address(0)) revert InvalidDeposit();
        assetRegistry = registry;
    }

    function setSettlementModule(address module, bool enabled) external onlyGovernance {
        if (module == address(0) || module.code.length == 0) {
            revert InvalidRelease();
        }
        settlementModule[module] = enabled;
        emit SettlementModuleSet(module, enabled);
    }

    function deposit(bytes32 assetId, uint256 amount, bytes32 beneficiary)
        external
        nonReentrant
        returns (bytes32 depositId)
    {
        ILayerXAssetRegistry.AssetConfig memory config = assetRegistry.asset(assetId);
        if (!config.enabled || config.paused || beneficiary == bytes32(0) || amount < config.minimumDeposit) {
            revert InvalidDeposit();
        }
        uint256 beforeBalance = SafeTransfer.balanceOf(config.token, address(this));
        SafeTransfer.safeTransferFrom(config.token, msg.sender, address(this), amount);
        uint256 afterBalance = SafeTransfer.balanceOf(config.token, address(this));
        if (afterBalance < beforeBalance || afterBalance - beforeBalance != amount || afterBalance > config.custodyCap)
        {
            revert InvalidDeposit();
        }
        uint64 nonce = ++depositNonce[msg.sender][assetId];
        depositId = sha256(
            abi.encode(
                "LXP/Paxeer/custody-deposit/v1",
                block.chainid,
                address(this),
                msg.sender,
                assetId,
                beneficiary,
                amount,
                nonce
            )
        );
        if (recordedDeposit[depositId]) revert InvalidDeposit();
        recordedDeposit[depositId] = true;
        totalCustodied[assetId] = afterBalance;
        emit CustodyDeposit(depositId, assetId, msg.sender, beneficiary, amount, nonce);
    }

    function release(bytes32 claimId, bytes32 assetId, address recipient, uint256 amount) external nonReentrant {
        if (!settlementModule[msg.sender]) revert SettlementModuleOnly();
        ILayerXAssetRegistry.AssetConfig memory config = assetRegistry.asset(assetId);
        if (
            claimId == bytes32(0) || recipient == address(0) || amount == 0 || !config.enabled || config.paused
                || releasedClaim[claimId]
        ) {
            revert InvalidRelease();
        }
        uint256 beforeBalance = SafeTransfer.balanceOf(config.token, address(this));
        if (amount > beforeBalance) revert InvalidRelease();
        releasedClaim[claimId] = true;
        totalReleased[assetId] = Arithmetic.add(totalReleased[assetId], amount);
        SafeTransfer.safeTransfer(config.token, recipient, amount);
        uint256 afterBalance = SafeTransfer.balanceOf(config.token, address(this));
        if (beforeBalance - afterBalance != amount) revert InvalidRelease();
        totalCustodied[assetId] = afterBalance;
        emit CustodyRelease(claimId, assetId, recipient, amount, msg.sender);
    }
}
