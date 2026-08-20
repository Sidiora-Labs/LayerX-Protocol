package com.sidiora.layerx.sdk;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.JavaType;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletionStage;

public final class ProductionClient {
    public record Options(IdempotencyKey idempotencyKey, Map<String, String> pathParameters) {
        public Options { pathParameters = pathParameters == null ? Map.of() : Map.copyOf(pathParameters); }
        public static Options none() { return new Options(null, Map.of()); }
        public static Options idempotent(IdempotencyKey key) { return new Options(key, Map.of()); }
    }
    public record TelemetryEvent(OperationCatalog.Plane plane, String operation, String outcome,
                                 PlatformSdkException.Code code) {}
    @FunctionalInterface public interface Telemetry { void record(TelemetryEvent event); }

    private final ProductionTransport transport;
    private final ObjectMapper mapper;
    private final Telemetry telemetry;

    public ProductionClient(ProductionTransport transport, ObjectMapper mapper, Telemetry telemetry) {
        this.transport = Objects.requireNonNull(transport, "transport");
        this.mapper = Objects.requireNonNull(mapper, "mapper");
        this.telemetry = telemetry;
    }
    public ProductionClient(ProductionTransport transport) { this(transport, new ObjectMapper(), null); }

    public <T> CompletionStage<T> agent(String operation, Object request, Class<T> responseType, Options options) {
        return execute(OperationCatalog.Plane.AGENT, operation, request, mapper.constructType(responseType), options);
    }
    public <T> CompletionStage<T> human(String operation, Object request, Class<T> responseType, Options options) {
        return execute(OperationCatalog.Plane.HUMAN, operation, request, mapper.constructType(responseType), options);
    }
    public <T> CompletionStage<T> agent(String operation, Object request, TypeReference<T> responseType, Options options) {
        return execute(OperationCatalog.Plane.AGENT, operation, request, mapper.constructType(responseType), options);
    }
    public <T> CompletionStage<T> human(String operation, Object request, TypeReference<T> responseType, Options options) {
        return execute(OperationCatalog.Plane.HUMAN, operation, request, mapper.constructType(responseType), options);
    }

    private <T> CompletionStage<T> execute(OperationCatalog.Plane plane, String operation, Object request,
                                            JavaType responseType, Options options) {
        OperationCatalog.requireKnown(plane, operation);
        Options safeOptions = options == null ? Options.none() : options;
        if (OperationCatalog.requiresIdempotency(plane, operation) && safeOptions.idempotencyKey() == null) {
            throw new PlatformSdkException(PlatformSdkException.Code.IDEMPOTENCY_REQUIRED,
                PlatformSdkException.Retry.NEVER, null, null, null);
        }
        try {
            return transport.<T>call(new ProductionTransport.Call(plane, operation, request,
                    safeOptions.pathParameters(), safeOptions.idempotencyKey()), responseType)
                .whenComplete((value, error) -> {
                    if (telemetry != null) telemetry.record(new TelemetryEvent(plane, operation,
                        error == null ? "completed" : "refused",
                        error instanceof PlatformSdkException sdk ? sdk.code() :
                            error == null ? null : PlatformSdkException.Code.TRANSPORT_FAILURE));
                });
        } catch (PlatformSdkException error) {
            throw error;
        } catch (RuntimeException error) {
            throw new PlatformSdkException(PlatformSdkException.Code.TRANSPORT_FAILURE,
                PlatformSdkException.Retry.SAFE, null, null, null);
        }
    }
}
