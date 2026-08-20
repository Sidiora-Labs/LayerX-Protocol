import Crypto
import Foundation
import LayerXSDK

public struct EventEnvelopeHeaders: Sendable, Equatable {
    public static let idHeader = "LayerX-Delivery-Id"
    public static let timestampHeader = "LayerX-Timestamp"
    public static let keyIDHeader = "LayerX-Key-Id"
    public static let signatureHeader = "LayerX-Signature"

    public let id: String
    public let timestamp: String
    public let keyID: String
    public let signature: String

    public init(id: String, timestamp: String, keyID: String, signature: String) {
        self.id = id
        self.timestamp = timestamp
        self.keyID = keyID
        self.signature = signature
    }

    public init(fields: [String: String]) throws {
        var normalized: [String: String] = [:]
        for (name, value) in fields {
            normalized[name.lowercased()] = value
        }
        guard let id = normalized[Self.idHeader.lowercased()],
              let timestamp = normalized[Self.timestampHeader.lowercased()],
              let keyID = normalized[Self.keyIDHeader.lowercased()],
              let signature = normalized[Self.signatureHeader.lowercased()] else {
            throw MobileIntegrationError(.invalidEvent)
        }
        self.init(id: id, timestamp: timestamp, keyID: keyID, signature: signature)
    }
}

public enum EventDeliveryClaim: String, Sendable {
    case claimed, processing, completed, conflict
}

public protocol EventDeliveryStore: Sendable {
    func claim(deliveryID: String, payloadDigest: String, leaseUntilMilliseconds: Int64) async throws -> EventDeliveryClaim
    func complete(deliveryID: String, payloadDigest: String) async throws
    func release(deliveryID: String, payloadDigest: String) async throws
}

public actor InMemoryEventDeliveryStore: EventDeliveryStore {
    private struct Entry {
        let payloadDigest: String
        var leaseUntilMilliseconds: Int64
        var completed: Bool
    }

    private var entries: [String: Entry] = [:]
    private let now: @Sendable () -> Int64
    private let capacity: Int

    public init(capacity: Int = 8_192, now: (@Sendable () -> Int64)? = nil) {
        self.capacity = max(capacity, 1)
        self.now = now ?? { Int64(Date().timeIntervalSince1970 * 1_000) }
    }

    public func claim(deliveryID: String, payloadDigest: String, leaseUntilMilliseconds: Int64) throws -> EventDeliveryClaim {
        if let existing = entries[deliveryID] {
            guard existing.payloadDigest == payloadDigest else { return .conflict }
            if existing.completed { return .completed }
            if existing.leaseUntilMilliseconds > now() { return .processing }
            entries[deliveryID] = Entry(payloadDigest: payloadDigest, leaseUntilMilliseconds: leaseUntilMilliseconds, completed: false)
            return .claimed
        }
        if entries.count >= capacity {
            evictOldest()
        }
        entries[deliveryID] = Entry(payloadDigest: payloadDigest, leaseUntilMilliseconds: leaseUntilMilliseconds, completed: false)
        return .claimed
    }

    public func complete(deliveryID: String, payloadDigest: String) throws {
        guard var entry = entries[deliveryID], entry.payloadDigest == payloadDigest else {
            throw MobileIntegrationError(.eventReplay)
        }
        entry.completed = true
        entry.leaseUntilMilliseconds = 0
        entries[deliveryID] = entry
    }

    public func release(deliveryID: String, payloadDigest: String) throws {
        guard let entry = entries[deliveryID], entry.payloadDigest == payloadDigest, !entry.completed else { return }
        entries.removeValue(forKey: deliveryID)
    }

    private func evictOldest() {
        let timestamp = now()
        for (identifier, entry) in entries where entry.completed || entry.leaseUntilMilliseconds <= timestamp {
            entries.removeValue(forKey: identifier)
        }
        if entries.count >= capacity, let victim = entries.keys.first {
            entries.removeValue(forKey: victim)
        }
    }
}

public enum EventConsumeOutcome: String, Sendable {
    case processed, duplicate, processing
}

public struct VerifiedEventConsumer: Sendable {
    private let publicKeys: [String: Data]
    private let deliveries: EventDeliveryStore
    private let maximumAgeMilliseconds: Int64
    private let leaseMilliseconds: Int64
    private let now: @Sendable () -> Int64

    public init(
        publicKeys: [String: Data],
        deliveries: EventDeliveryStore,
        maximumAgeMilliseconds: Int64 = 300_000,
        leaseMilliseconds: Int64 = 60_000,
        now: (@Sendable () -> Int64)? = nil
    ) throws {
        guard !publicKeys.isEmpty, maximumAgeMilliseconds > 0, leaseMilliseconds > 0,
              publicKeys.allSatisfy({ $0.value.count == 32 }) else {
            throw MobileIntegrationError(.invalidConfiguration)
        }
        self.publicKeys = publicKeys
        self.deliveries = deliveries
        self.maximumAgeMilliseconds = maximumAgeMilliseconds
        self.leaseMilliseconds = leaseMilliseconds
        self.now = now ?? { Int64(Date().timeIntervalSince1970 * 1_000) }
    }

    public init(configuration: PublishableConfiguration, deliveries: EventDeliveryStore) throws {
        try self.init(
            publicKeys: configuration.eventPublicKeys,
            deliveries: deliveries,
            maximumAgeMilliseconds: configuration.eventMaximumAgeMilliseconds
        )
    }

    @discardableResult
    public func consume(
        rawBody: Data,
        headers: EventEnvelopeHeaders,
        handle: (JSONValue, String) async throws -> Void
    ) async throws -> EventConsumeOutcome {
        let timestamp = now()
        let seconds = try canonicalSeconds(headers.timestamp)
        let issuedAt = seconds * 1_000
        guard boundedText(headers.id, limit: 255),
              identifier(headers.keyID, limit: 64),
              rawBody.count <= 1_048_576,
              issuedAt <= timestamp + 30_000,
              timestamp - issuedAt <= maximumAgeMilliseconds,
              let publicKey = publicKeys[headers.keyID] else {
            throw MobileIntegrationError(.invalidEvent)
        }
        let signature = try parseSignature(headers.signature)
        var message = Data("\(headers.id).\(headers.timestamp).".utf8)
        message.append(rawBody)
        guard let key = try? Curve25519.Signing.PublicKey(rawRepresentation: publicKey),
              key.isValidSignature(signature, for: message) else {
            throw MobileIntegrationError(.invalidEvent)
        }

        let payloadDigest = hexadecimal(Data(SHA256.hash(data: rawBody)))
        let claim = try await deliveries.claim(
            deliveryID: headers.id,
            payloadDigest: payloadDigest,
            leaseUntilMilliseconds: timestamp + leaseMilliseconds
        )
        switch claim {
        case .conflict:
            throw MobileIntegrationError(.eventReplay)
        case .completed:
            return .duplicate
        case .processing:
            return .processing
        case .claimed:
            break
        }

        do {
            let event = try decodeEvent(rawBody)
            try await handle(event, headers.id)
            try await deliveries.complete(deliveryID: headers.id, payloadDigest: payloadDigest)
        } catch {
            try? await deliveries.release(deliveryID: headers.id, payloadDigest: payloadDigest)
            throw error
        }
        return .processed
    }

    private func decodeEvent(_ rawBody: Data) throws -> JSONValue {
        guard let value = try? JSONDecoder().decode(JSONValue.self, from: rawBody), value.objectValue != nil else {
            throw MobileIntegrationError(.decodeFailure)
        }
        return value
    }

    private func canonicalSeconds(_ value: String) throws -> Int64 {
        guard !value.isEmpty, value.utf8.count <= 19,
              value.utf8.allSatisfy({ $0 >= 48 && $0 <= 57 }),
              value == "0" || value.first != "0",
              let seconds = Int64(value), seconds <= 253_402_300_799 else {
            throw MobileIntegrationError(.invalidEvent)
        }
        return seconds
    }

    private func parseSignature(_ value: String) throws -> Data {
        guard value.hasPrefix("v1="),
              let decoded = Data(base64Encoded: String(value.dropFirst(3))),
              decoded.count == 64 else {
            throw MobileIntegrationError(.invalidEvent)
        }
        return decoded
    }

    private func boundedText(_ value: String, limit: Int) -> Bool {
        !value.isEmpty && value.utf8.count <= limit && !value.contains("\0")
    }

    private func identifier(_ value: String, limit: Int) -> Bool {
        boundedText(value, limit: limit) && value.allSatisfy { character in
            character.isASCII && (character.isLetter || character.isNumber || character == "." || character == "_" || character == "-")
        }
    }

    private func hexadecimal(_ bytes: Data) -> String {
        bytes.map { String(format: "%02x", $0) }.joined()
    }
}
