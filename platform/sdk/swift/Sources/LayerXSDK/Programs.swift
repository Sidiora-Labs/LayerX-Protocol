import Foundation

public struct ProgramBudget: Sendable {
    public let fuel: UInt64
    public let feeLimit: ProtocolAmount
    public init(fuel: UInt64, feeLimit: ProtocolAmount) { self.fuel = fuel; self.feeLimit = feeLimit }
}

public struct ProgramCall: Sendable {
    public let programID: Data; public let version: UInt32; public let codeHash: Data; public let abiVersion: UInt16
    public let entrypoint: String; public let calldata: Data; public let budget: ProgramBudget
    public let capabilities: [Data]; public let signedActivity: Data

    public init(programID: Data, version: UInt32, codeHash: Data, abiVersion: UInt16, entrypoint: String,
                calldata: Data, budget: ProgramBudget, capabilities: [Data], signedActivity: Data) throws {
        guard programID.count == 32, version > 0, codeHash.count == 32, abiVersion > 0,
              !entrypoint.isEmpty, entrypoint.utf8.count <= 255, budget.fuel <= UInt64(Int64.max), calldata.count <= 1_048_576,
              capabilities.count <= 256, capabilities.allSatisfy({ !$0.isEmpty && $0.count <= 4_096 }),
              zip(capabilities, capabilities.dropFirst()).allSatisfy({ $0.lexicographicallyPrecedes($1) }),
              !signedActivity.isEmpty else { throw programInvalid() }
        self.programID = programID; self.version = version; self.codeHash = codeHash; self.abiVersion = abiVersion
        self.entrypoint = entrypoint; self.calldata = calldata; self.budget = budget
        self.capabilities = capabilities; self.signedActivity = signedActivity
    }
}

public struct ProgramDiscovery: Sendable { public let value: JSONValue }
public struct ProgramInterface: Sendable { public let value: JSONValue }
public struct ProgramSimulation: Sendable { public let value: JSONValue }
public struct ProgramSubmission: Sendable {
    public let value: JSONValue
    public let state: String
    public var isUnknown: Bool { state == "unknown" }
    fileprivate init(_ value: JSONValue) throws {
        guard let state = value.objectValue?["state"]?.stringValue,
              ["refused", "unknown", "executed"].contains(state) else { throw programVerification() }
        self.value = value; self.state = state
    }
}

public struct ProgramsClient: Sendable {
    public static let receiptModuleID: UInt16 = 9
    public static let callOperation: UInt8 = 3
    private let client: PlatformClient
    public init(client: PlatformClient) { self.client = client }

    public func discover(programID: Data, verificationLevel: String) async throws -> ProgramDiscovery {
        let id = try identifier(programID); return .init(value: try await client.agentProgramDiscover(
            .object(["program_id": .string(id), "requested_verification_level": .string(try level(verificationLevel))]),
            pathParameters: ["program_id": id]))
    }
    public func interface(programID: Data, version: UInt32, verificationLevel: String) async throws -> ProgramInterface {
        guard version > 0 else { throw programInvalid() }; let id = try identifier(programID)
        return .init(value: try await client.agentProgramInterface(.object(["program_id": .string(id),
            "version": .integer(Int64(version)), "requested_verification_level": .string(try level(verificationLevel))]),
            pathParameters: ["program_id": id]))
    }
    public func simulate(_ call: ProgramCall) async throws -> ProgramSimulation {
        .init(value: try await client.agentProgramSimulate(encode(call)))
    }
    public func submit(_ call: ProgramCall, idempotencyKey: IdempotencyKey) async throws -> ProgramSubmission {
        try .init(await client.agentProgramCall(encode(call), idempotencyKey: idempotencyKey))
    }
    public func receipt(idempotencyKey: IdempotencyKey, expectedActivityID: Data, verificationLevel: String) async throws -> ProgramSubmission {
        let activity = try identifier(expectedActivityID)
        return try .init(await client.agentProgramReceipt(.object(["idempotency_key": .string(idempotencyKey.rawValue),
            "expected_activity_id": .string(activity), "requested_verification_level": .string(try level(verificationLevel))]),
            pathParameters: ["idempotency_key": idempotencyKey.rawValue]))
    }
    public func activity(activityID: Data, verificationLevel: String) async throws -> ProgramSubmission {
        let id = try identifier(activityID)
        return try .init(await client.agentProgramActivity(.object(["activity_id": .string(id),
            "requested_verification_level": .string(try level(verificationLevel))]), pathParameters: ["activity_id": id]))
    }

    public static func verifyReceipt(_ canonicalReceipt: Data, authorized: AuthorizedReceiptBatch,
                                     expectedActivityID: Data, version: UInt32, abiVersion: UInt16) async throws -> ReceiptVerification {
        guard expectedActivityID.count == 32, version > 0 else { throw programInvalid() }
        let verified = try await LocalVerifier.verifyReceiptOutcome(canonicalReceipt, authorized: authorized)
        let receipt = verified.receipt
        guard receipt.protocolVersion > 0, receipt.moduleID == receiptModuleID, receipt.operation == callOperation,
              receipt.moduleVersion == UInt32(abiVersion),
              receipt.activityID == expectedActivityID else { throw programVerification() }
        return verified
    }
}

private func encode(_ call: ProgramCall) -> JSONValue {
    .object(["program_id": .string(call.programID.hex), "version": .integer(Int64(call.version)),
        "code_hash": .string(call.codeHash.hex), "abi_version": .integer(Int64(call.abiVersion)),
        "entrypoint": .string(call.entrypoint), "calldata": .string(call.calldata.base64EncodedString()),
        "budget": .object(["fuel": .integer(Int64(call.budget.fuel)), "fee_limit": .string(call.budget.feeLimit.decimal)]),
        "capabilities": .array(call.capabilities.map { .string($0.base64EncodedString()) }),
        "signed_activity": .string(call.signedActivity.base64EncodedString())])
}
private func identifier(_ value: Data) throws -> String { guard value.count == 32 else { throw programInvalid() }; return value.hex }
private func level(_ value: String) throws -> String { guard !value.isEmpty, value.utf8.count <= 64 else { throw programInvalid() }; return value }
private func programInvalid() -> PlatformSDKError { .init(code: .invalidArgument, retry: .never) }
private func programVerification() -> PlatformSDKError { .init(code: .verificationFailure, retry: .never) }
private extension Data { var hex: String { map { String(format: "%02x", $0) }.joined() } }
