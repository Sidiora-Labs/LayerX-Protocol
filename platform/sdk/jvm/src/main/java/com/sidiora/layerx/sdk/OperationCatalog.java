package com.sidiora.layerx.sdk;

import java.util.List;
import java.util.Map;
import java.util.Set;

/** Schema-generated operation inventory shared by the Java and Kotlin APIs. */
public final class OperationCatalog {
    private OperationCatalog() {}
    public enum Plane { AGENT, HUMAN }
    public record Route(String method, String path, List<String> pathParameters,
                        boolean idempotency, boolean bodyless) {
        public Route { pathParameters = List.copyOf(pathParameters); }
    }

    public static final Set<String> AGENT_OPERATIONS = GeneratedContract.AGENT_OPERATIONS;
    public static final Set<String> AGENT_IDEMPOTENT = GeneratedContract.AGENT_IDEMPOTENT;
    public static final Set<String> HUMAN_ERROR_CODES = GeneratedContract.HUMAN_ERROR_CODES;
    public static final Map<String, Route> HUMAN_ROUTES = GeneratedContract.HUMAN_ROUTES;

    public static boolean requiresIdempotency(Plane plane, String operation) {
        Route human = HUMAN_ROUTES.get(operation);
        return plane == Plane.AGENT ? AGENT_IDEMPOTENT.contains(operation)
            : human != null && human.idempotency();
    }

    public static void requireKnown(Plane plane, String operation) {
        boolean known = plane == Plane.AGENT ? AGENT_OPERATIONS.contains(operation)
            : HUMAN_ROUTES.containsKey(operation);
        if (!known) throw PlatformSdkException.invalidArgument();
    }
}
