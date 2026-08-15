from __future__ import annotations

import socket
import sys

from layerx_sdk import (
    ApiError,
    Client,
    SubmissionFailed,
    SubmissionUnknown,
    VerificationLevel,
    VerifiedRead,
    require_verified,
)


class ParityTransport:
    def __init__(self, path: str) -> None:
        self.path = path

    def call(self, operation: str, request: object) -> object:
        if operation != "track" or not isinstance(request, dict):
            raise ValueError("invalid parity request")
        scenario = request.get("scenario")
        if not isinstance(scenario, str):
            raise ValueError("missing parity scenario")
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as connection:
            connection.connect(self.path)
            connection.sendall(f"python\t{scenario}\n".encode())
            chunks: list[bytes] = []
            while chunk := connection.recv(4096):
                chunks.append(chunk)
        return b"".join(chunks).decode().strip()


def fields(encoded: str) -> dict[str, str]:
    return dict(field.split("=", 1) for field in encoded.split(";"))


def validate(scenario: str, encoded: str) -> None:
    value = fields(encoded)
    if scenario == "unknown_submission":
        if SubmissionUnknown().kind != value["state"]:
            raise RuntimeError("unknown submission collapsed")
    elif scenario == "terminal_rejection":
        state = SubmissionFailed(int(value["result_code"]))
        error = ApiError("CoreRejection", state.protocol_result_code, False, 2, "terminal")
        if state.protocol_result_code != -77_777 or error.error_class != value["error"]:
            raise RuntimeError("terminal result code changed")
    elif scenario == "proven_read":
        read = VerifiedRead(1, VerificationLevel.STATE_PROVEN, 10, "22", "genesis", 10)
        require_verified(VerificationLevel.STATE_PROVEN, read)
        if value["verification"] != "StateProven":
            raise RuntimeError("proven read level changed")
    elif scenario == "availability_failure":
        error = ApiError("UnavailableCapability", None, False, 18, "capability_absent")
        if error.error_class != value["error"]:
            raise RuntimeError("availability error changed")
    elif scenario == "subscription_gap" and value["state"] != "Gap":
        raise RuntimeError("subscription gap hidden")
    elif scenario.startswith("idempotency_"):
        if value["receipt_count"] != "1" or value["economic_effects"] != "1":
            raise RuntimeError("idempotency duplicated an effect")


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: python.py SOCKET SCENARIOS")
    client = Client(ParityTransport(sys.argv[1]))
    for scenario in sys.argv[2].split(","):
        encoded = client.call("track", {"scenario": scenario})
        if not isinstance(encoded, str):
            raise RuntimeError("invalid parity response")
        validate(scenario, encoded)
        print(f"{scenario}\t{encoded}")


if __name__ == "__main__":
    main()
