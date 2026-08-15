from __future__ import annotations

import ast
import importlib.util
from pathlib import Path
import tempfile
from types import MappingProxyType
from typing import get_args
import unittest
import zipfile

from layerx_sdk import (
    ApiError,
    ErrorClass,
    IdempotentMutation,
    SubmissionFailed,
    SubmissionUnknown,
    VerificationLevel,
    VerifiedRead,
    layerx_sdk_py_package,
    parse_amount,
    parse_sequence,
    require_verified,
)


ROOT = Path(__file__).resolve().parents[1]


def load_build_backend():
    spec = importlib.util.spec_from_file_location("layerx_sdk_build_backend", ROOT / "build_backend.py")
    if spec is None or spec.loader is None:
        raise RuntimeError("build backend unavailable")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class PythonSdkIntegration(unittest.TestCase):
    def test_package_metadata_is_immutable_and_contract_bound(self) -> None:
        metadata = layerx_sdk_py_package()
        self.assertIsInstance(metadata, MappingProxyType)
        self.assertEqual(metadata["name"], "layerx-sdk")
        self.assertEqual(metadata["version"], "0.1.0")
        self.assertEqual(metadata["contract_major"], 1)
        with self.assertRaises(TypeError):
            metadata["contract_major"] = 2  # type: ignore[index]

    def test_consensus_integers_are_exact_and_bounded(self) -> None:
        self.assertEqual(parse_amount("9007199254740993"), 9_007_199_254_740_993)
        self.assertEqual(parse_sequence("18446744073709551615"), 18_446_744_073_709_551_615)
        for invalid in ("", "01", "-1", "1.5", "１"):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                parse_amount(invalid)
        with self.assertRaises(OverflowError):
            parse_sequence("18446744073709551616")

    def test_reads_require_the_declared_verification_level(self) -> None:
        read = VerifiedRead(
            value=10,
            achieved_verification_level=VerificationLevel.STATE_PROVEN,
            chain_head=20,
            latest_batch="batch-19",
            latest_checkpoint="checkpoint-18",
            value_sequence=17,
        )
        self.assertIs(require_verified(VerificationLevel.STATE_PROVEN, read), read)
        with self.assertRaisesRegex(ValueError, "verification_below_requested"):
            require_verified(VerificationLevel.CHECKPOINT_FINALISED, read)
        unverified = VerifiedRead(
            value=10,
            achieved_verification_level=VerificationLevel.UNVERIFIED,
            chain_head=20,
            latest_batch="batch-19",
            latest_checkpoint="checkpoint-18",
            value_sequence=17,
        )
        with self.assertRaisesRegex(ValueError, "unverified_read"):
            require_verified(VerificationLevel.UNVERIFIED, unverified)

    def test_unknown_idempotency_and_future_result_codes_remain_lossless(self) -> None:
        unknown = SubmissionUnknown()
        future_failure = SubmissionFailed(protocol_result_code=-77_777)
        mutation = IdempotentMutation(
            request_id=18_446_744_073_709_551_615,
            key=bytes(range(32)),
            body_digest=bytes(reversed(range(32))),
            operation={"amount": 9_007_199_254_740_993},
        )
        self.assertEqual(unknown.kind, "Unknown")
        self.assertEqual(future_failure.protocol_result_code, -77_777)
        self.assertEqual(mutation.operation["amount"], 9_007_199_254_740_993)
        self.assertEqual(len(mutation.key), 32)

    def test_typed_error_taxonomy_is_complete_and_disjoint(self) -> None:
        classes = set(get_args(ErrorClass))
        self.assertEqual(len(classes), 12)
        self.assertIn("TransportFailure", classes)
        self.assertIn("VerificationFailure", classes)
        self.assertNotEqual("TransportFailure", "VerificationFailure")
        error = ApiError("CoreRejection", -77_777, False, 7, "future_code")
        self.assertEqual(error.protocol_result_code, -77_777)
        self.assertEqual(str(error), "CoreRejection:future_code")

    def test_type_stubs_and_examples_are_parseable(self) -> None:
        for path in sorted((ROOT / "layerx_sdk").rglob("*.pyi")):
            ast.parse(path.read_text(), filename=str(path))
        examples = sorted((ROOT / "examples").glob("*.py"))
        self.assertEqual(len(examples), 5)
        for path in examples:
            ast.parse(path.read_text(), filename=str(path))

    def test_wheel_build_is_byte_reproducible_and_complete(self) -> None:
        backend = load_build_backend()
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            first_name = backend.build_wheel(first)
            second_name = backend.build_wheel(second)
            first_bytes = (Path(first) / first_name).read_bytes()
            second_bytes = (Path(second) / second_name).read_bytes()
            self.assertEqual(first_bytes, second_bytes)
            with zipfile.ZipFile(Path(first) / first_name) as archive:
                names = set(archive.namelist())
                self.assertIn("layerx_sdk/generated/client.py", names)
                self.assertIn("layerx_sdk/generated/client.pyi", names)
                self.assertIn("layerx_sdk/py.typed", names)
                self.assertEqual(
                    len([name for name in names if "/share/layerx-sdk/examples/" in name]),
                    5,
                )
                metadata = archive.read("layerx_sdk-0.1.0.dist-info/METADATA").decode()
                self.assertIn("Requires-Python: >=3.11", metadata)


if __name__ == "__main__":
    unittest.main()
