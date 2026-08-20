package com.sidiora.layerx.sdk;

import com.fasterxml.jackson.annotation.JsonValue;
import java.util.Map;

public final class PlatformSdkException extends RuntimeException {
    public enum Code {
        INVALID_ARGUMENT("invalid-argument"), IDEMPOTENCY_REQUIRED("idempotency-required"),
        TRANSPORT_FAILURE("transport-failure"), DEADLINE("deadline"),
        PROTOCOL_INCOMPATIBILITY("protocol-incompatibility"), UNAVAILABLE_CAPABILITY("unavailable-capability"),
        CORE_REJECTION("core-rejection"), VERIFICATION_FAILURE("verification-failure"),
        POLICY_REFUSAL("policy-refusal"), CAPABILITY_REFUSAL("capability-refusal"),
        BUDGET_REFUSAL("budget-refusal"), RATE_LIMIT("rate-limit"),
        IDEMPOTENCY_CONFLICT("idempotency-conflict"), DECODE_FAILURE("decode-failure"),
        UNKNOWN_OUTCOME("unknown-outcome"), INTERNAL_FAULT("internal-fault");
        private final String wire;
        Code(String wire) { this.wire = wire; }
        public String wire() { return wire; }
    }
    public enum Retry { NEVER("never"), SAFE("safe"), AFTER("after"), UNKNOWN_OUTCOME("unknown-outcome");
        private final String wire;
        Retry(String wire) { this.wire = wire; }
        public String wire() { return wire; }
    }

    private static final Map<Code, String> MESSAGES = Map.ofEntries(
        Map.entry(Code.INVALID_ARGUMENT, "The SDK rejected an invalid argument."),
        Map.entry(Code.IDEMPOTENCY_REQUIRED, "This operation requires an idempotency key."),
        Map.entry(Code.TRANSPORT_FAILURE, "The request could not reach the service."),
        Map.entry(Code.DEADLINE, "The request deadline elapsed."),
        Map.entry(Code.PROTOCOL_INCOMPATIBILITY, "The service protocol is not compatible with this SDK."),
        Map.entry(Code.UNAVAILABLE_CAPABILITY, "The requested operation is unavailable."),
        Map.entry(Code.CORE_REJECTION, "The protocol refused the request."),
        Map.entry(Code.VERIFICATION_FAILURE, "Local verification failed."),
        Map.entry(Code.POLICY_REFUSAL, "Policy refused the request."),
        Map.entry(Code.CAPABILITY_REFUSAL, "The caller does not have the required authority."),
        Map.entry(Code.BUDGET_REFUSAL, "The configured budget refused the request."),
        Map.entry(Code.RATE_LIMIT, "The request rate limit was reached."),
        Map.entry(Code.IDEMPOTENCY_CONFLICT, "The idempotency key belongs to a different request."),
        Map.entry(Code.DECODE_FAILURE, "The service response did not match the contract."),
        Map.entry(Code.UNKNOWN_OUTCOME, "The request outcome is unknown and must be resolved before retrying."),
        Map.entry(Code.INTERNAL_FAULT, "The service could not complete the request."));

    private final Code code;
    private final Retry retry;
    private final String requestId;
    private final Integer protocolResultCode;
    private final Long retryAfterMs;

    public PlatformSdkException(Code code, Retry retry, String requestId, Integer protocolResultCode, Long retryAfterMs) {
        super(MESSAGES.get(code));
        this.code = code;
        this.retry = retry;
        this.requestId = requestId;
        this.protocolResultCode = protocolResultCode;
        this.retryAfterMs = retryAfterMs;
    }

    public static PlatformSdkException invalidArgument() {
        return new PlatformSdkException(Code.INVALID_ARGUMENT, Retry.NEVER, null, null, null);
    }
    public static PlatformSdkException verificationFailure() {
        return new PlatformSdkException(Code.VERIFICATION_FAILURE, Retry.NEVER, null, null, null);
    }
    public Code code() { return code; }
    public Retry retry() { return retry; }
    public String requestId() { return requestId; }
    public Integer protocolResultCode() { return protocolResultCode; }
    public Long retryAfterMs() { return retryAfterMs; }
    @JsonValue public Map<String, Object> safeDetails() {
        var value = new java.util.LinkedHashMap<String, Object>();
        value.put("code", code.wire()); value.put("retry", retry.wire());
        if (requestId != null) value.put("requestId", requestId);
        if (protocolResultCode != null) value.put("protocolResultCode", protocolResultCode);
        if (retryAfterMs != null) value.put("retryAfterMs", retryAfterMs);
        return Map.copyOf(value);
    }
}
