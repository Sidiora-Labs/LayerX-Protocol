// SPDX-License-Identifier: LicenseRef-Centra-ai-Protocol
pragma solidity ^0.8.24;

import {CanonicalCheckpoint} from "../contracts/libraries/CanonicalCheckpoint.sol";
import {GuarantorBond} from "../contracts/GuarantorBond.sol";

interface BondVm {
    function addr(uint256 privateKey) external returns (address);
    function deal(address account, uint256 balance) external;
    function prank(address sender) external;
    function sign(uint256 privateKey, bytes32 digest) external returns (uint8 v, bytes32 r, bytes32 s);
}

contract GuarantorBondTest {
    BondVm private constant vm = BondVm(address(uint160(uint256(keccak256("hevm cheat code")))));
    bytes32 private constant CONFIG = keccak256("bond-test-config");
    uint192 private constant RELEASE = uint192(1) << 128;

    function testPermissionlessPortableEquivocationSlashes() public {
        uint256 privateKey = 7;
        address signer = vm.addr(privateKey);
        bytes32 guarantorId = bytes32(uint256(7));
        GuarantorBond bond = new GuarantorBond(address(this), 100, 100 ether, 7 days, CONFIG, RELEASE);
        vm.deal(signer, 2 ether);
        vm.prank(signer);
        bond.depositBond{value: 1 ether}(guarantorId, 1);
        CanonicalCheckpoint.GuarantorAttestation memory first = _statement(guarantorId, signer, bytes32(uint256(1)));
        CanonicalCheckpoint.GuarantorAttestation memory second = _statement(guarantorId, signer, bytes32(uint256(2)));
        _sign(first, privateKey);
        _sign(second, privateKey);
        vm.prank(address(0xBEEF));
        bond.submitEquivocation(first, second, 4);
        GuarantorBond.BondRecord memory record = bond.bondRecord(guarantorId);
        require(record.jailed && record.amount == 0 && record.removedEpoch == 4, "not slashed");
        require(bond.slashedBalance() == 1 ether, "slash not conserved");
    }

    function testUnbondingImmediatelyRemovesEligibility() public {
        uint256 privateKey = 8;
        address signer = vm.addr(privateKey);
        bytes32 guarantorId = bytes32(uint256(8));
        GuarantorBond bond = new GuarantorBond(address(this), 100, 100 ether, 7 days, CONFIG, RELEASE);
        vm.deal(signer, 2 ether);
        vm.prank(signer);
        bond.depositBond{value: 1 ether}(guarantorId, 1);
        require(bond.bondedActive(guarantorId, signer, 1), "not active");
        vm.prank(signer);
        bond.beginUnbond(guarantorId, 1 ether);
        require(!bond.bondedActive(guarantorId, signer, 1), "unbonding signer remained active");
    }

    function _statement(bytes32 guarantorId, address signer, bytes32 checkpointHash)
        private
        pure
        returns (CanonicalCheckpoint.GuarantorAttestation memory statement)
    {
        statement = CanonicalCheckpoint.GuarantorAttestation({
            checkpointId: bytes32(uint256(99)),
            checkpointHash: checkpointHash,
            guarantorId: guarantorId,
            batchNumber: 12,
            dataAvailabilityRoot: bytes32(uint256(3)),
            replayed: true,
            dataAvailable: true,
            availabilityClassMask: 0x1f,
            attestedAt: 100,
            signer: signer,
            r: bytes32(0),
            s: bytes32(0),
            v: 0
        });
    }

    function _sign(CanonicalCheckpoint.GuarantorAttestation memory statement, uint256 privateKey) private {
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(privateKey, CanonicalCheckpoint.attestationHash(statement));
        statement.v = v;
        statement.r = r;
        statement.s = s;
    }
}
