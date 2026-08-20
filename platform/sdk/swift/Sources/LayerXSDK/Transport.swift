import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

private let maximumHTTPResponseBytes = 8 * 1024 * 1024

public final class AccessToken: @unchecked Sendable, CustomStringConvertible {
    private let secret: SecretBytes

    public init(_ bytes: Data) throws {
        guard String(data: bytes, encoding: .utf8) != nil else {
            throw PlatformSDKError(code: .invalidArgument, retry: .never)
        }
        secret = try SecretBytes(bytes)
    }

    fileprivate func authorize(_ request: inout URLRequest) throws {
        try secret.withBytes { bytes in
            guard let value = String(data: bytes, encoding: .utf8), !value.contains("\r"), !value.contains("\n") else {
                throw PlatformSDKError(code: .invalidArgument, retry: .never)
            }
            request.setValue("Bearer \(value)", forHTTPHeaderField: "Authorization")
        }
    }

    public func destroy() { secret.destroy() }
    public var description: String { "[REDACTED]" }
}

public final class HumanHTTPTransport: PlatformTransport, @unchecked Sendable {
    private let baseURL: URL
    private let session: URLSession
    private let accessToken: AccessToken?

    public init(baseURL: URL, session: URLSession = .shared, accessToken: AccessToken? = nil) throws {
        guard baseURL.user == nil, baseURL.password == nil, baseURL.host != nil,
              baseURL.scheme == "https" || (baseURL.scheme == "http" && Self.isLoopback(baseURL.host)) else {
            throw PlatformSDKError(code: .invalidArgument, retry: .never)
        }
        self.baseURL = baseURL
        self.session = session
        self.accessToken = accessToken
    }

    public func send(_ call: TransportCall) async throws -> JSONValue {
        let descriptor = call.operation.descriptor
        guard descriptor.plane == .human else {
            throw PlatformSDKError(code: .unavailableCapability, retry: .never)
        }
        let path = try Self.resolvePath(descriptor.path, parameters: call.pathParameters)
        guard let target = URL(string: path, relativeTo: baseURL)?.absoluteURL,
              target.scheme == baseURL.scheme, target.host == baseURL.host, target.port == baseURL.port else {
            throw PlatformSDKError(code: .invalidArgument, retry: .never)
        }
        var request = URLRequest(url: target)
        request.httpMethod = descriptor.method.rawValue
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("layerx-swift/0.1.0", forHTTPHeaderField: "User-Agent")
        if !descriptor.bodyless {
            request.httpBody = try JSONEncoder().encode(call.request)
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        }
        if let key = call.idempotencyKey {
            request.setValue(key.rawValue, forHTTPHeaderField: "Idempotency-Key")
        }
        try accessToken?.authorize(&request)

        let (data, response) = try await perform(request)
        guard data.count <= maximumHTTPResponseBytes, let http = response as? HTTPURLResponse else {
            throw PlatformSDKError(code: .decodeFailure, retry: .never)
        }
        let envelope: HumanEnvelope
        do {
            envelope = try JSONDecoder().decode(HumanEnvelope.self, from: data)
        } catch {
            throw PlatformSDKError(code: .decodeFailure, retry: .never)
        }
        guard !envelope.trace.isEmpty, envelope.trace.utf8.count <= 512,
              !envelope.trace.contains("\0"), !envelope.trace.contains("\r"), !envelope.trace.contains("\n") else {
            throw PlatformSDKError(code: .decodeFailure, retry: .never)
        }
        if envelope.ok {
            guard (200..<300).contains(http.statusCode), envelope.error == nil, let result = envelope.result else {
                throw PlatformSDKError(code: .decodeFailure, retry: .never)
            }
            return result
        }
        guard !(200..<300).contains(http.statusCode), envelope.result == nil, let error = envelope.error else {
            throw PlatformSDKError(code: .decodeFailure, retry: .never)
        }
        throw error.sdkError(trace: envelope.trace)
    }

    private func perform(_ request: URLRequest) async throws -> (Data, URLResponse) {
        let box = URLSessionTaskBox()
        return try await withTaskCancellationHandler(operation: {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<(Data, URLResponse), Error>) in
                let task = session.dataTask(with: request) { data, response, error in
                    if let error { continuation.resume(throwing: error); return }
                    guard let data, let response else {
                        continuation.resume(throwing: PlatformSDKError(code: .transportFailure, retry: .safe))
                        return
                    }
                    continuation.resume(returning: (data, response))
                }
                box.install(task)
                task.resume()
            }
        }, onCancel: {
            box.cancel()
        })
    }

    private static func resolvePath(_ template: String, parameters: [String: String]) throws -> String {
        var path = template
        for (name, value) in parameters {
            guard !name.isEmpty, !value.isEmpty, !name.contains("{") && !name.contains("}") else {
                throw PlatformSDKError(code: .invalidArgument, retry: .never)
            }
            path = path.replacingOccurrences(of: "{\(name)}", with: percentEncodePathSegment(value))
        }
        guard !path.contains("{") && !path.contains("}") && path.first == "/" else {
            throw PlatformSDKError(code: .invalidArgument, retry: .never)
        }
        return path
    }

    private static func percentEncodePathSegment(_ value: String) -> String {
        let allowed = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
        return value.addingPercentEncoding(withAllowedCharacters: allowed) ?? ""
    }

    private static func isLoopback(_ host: String?) -> Bool {
        guard let host = host?.lowercased() else { return false }
        return host == "localhost" || host == "127.0.0.1" || host == "::1" || host == "[::1]"
    }
}

private final class URLSessionTaskBox: @unchecked Sendable {
    private let lock = NSLock()
    private var task: URLSessionTask?
    private var cancelled = false

    func install(_ task: URLSessionTask) {
        lock.lock()
        self.task = task
        let shouldCancel = cancelled
        lock.unlock()
        if shouldCancel { task.cancel() }
    }

    func cancel() {
        lock.lock()
        cancelled = true
        let task = task
        lock.unlock()
        task?.cancel()
    }
}

private struct HumanEnvelope: Decodable {
    let ok: Bool
    let result: JSONValue?
    let error: HumanAPIError?
    let trace: String
}

private struct HumanAPIError: Decodable {
    let code: String
    let retry: String
    let retryAfterMilliseconds: UInt64?

    enum CodingKeys: String, CodingKey {
        case code, retry
        case retryAfterMilliseconds = "retry_after_ms"
    }

    func sdkError(trace: String) -> PlatformSDKError {
        let mapped: SDKErrorCode
        switch code {
        case "rate-limited": mapped = .rateLimit
        case "unavailable", "upstream-degraded": mapped = .transportFailure
        case "refused-by-policy": mapped = .policyRefusal
        case "refused-by-budget", "refused-by-limit": mapped = .budgetRefusal
        case "refused-by-capability", "forbidden", "unauthenticated", "session-expired", "step-up-required": mapped = .capabilityRefusal
        case "conflict": mapped = .idempotencyConflict
        case "refused-by-protocol": mapped = .coreRejection
        default: mapped = .coreRejection
        }
        let retryClass: RetryClass
        switch retry {
        case "retriable": retryClass = .safe
        case "retriable-after":
            guard retryAfterMilliseconds != nil else { return PlatformSDKError(code: .decodeFailure, retry: .never) }
            retryClass = .after
        case "structural", "final": retryClass = .never
        default: return PlatformSDKError(code: .decodeFailure, retry: .never)
        }
        return PlatformSDKError(code: mapped, retry: retryClass, requestID: trace, retryAfterMilliseconds: retryAfterMilliseconds)
    }
}
