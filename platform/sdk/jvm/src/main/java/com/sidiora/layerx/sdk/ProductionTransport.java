package com.sidiora.layerx.sdk;

import com.fasterxml.jackson.databind.JavaType;
import java.util.Map;
import java.util.concurrent.CompletionStage;

public interface ProductionTransport {
    record Call(OperationCatalog.Plane plane, String operation, Object request,
                Map<String, String> pathParameters, IdempotencyKey idempotencyKey) {
        public Call {
            pathParameters = pathParameters == null ? Map.of() : Map.copyOf(pathParameters);
        }
    }
    <T> CompletionStage<T> call(Call call, JavaType responseType);
}
