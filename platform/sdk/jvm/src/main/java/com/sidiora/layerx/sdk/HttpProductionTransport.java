package com.sidiora.layerx.sdk;

import com.fasterxml.jackson.databind.JavaType;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.IOException;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.CompletionStage;

/** HTTP/JSON transport for the schema-defined human routes and the agent RPC endpoint. */
public final class HttpProductionTransport implements ProductionTransport {
    private static final int MAXIMUM_RESPONSE_BYTES = 8 * 1024 * 1024;
    @FunctionalInterface public interface Credential { void apply(HttpRequest.Builder request); }

    public static final class BearerCredential implements Credential, AutoCloseable {
        private final SecretBytes token;
        public BearerCredential(SecretBytes token) { this.token = Objects.requireNonNull(token, "token"); }
        @Override public void apply(HttpRequest.Builder request) {
            token.use(bytes -> {
                request.header("Authorization", "Bearer " + new String(bytes, StandardCharsets.UTF_8));
                return null;
            });
        }
        @Override public void close() { token.close(); }
        @Override public String toString() { return "[REDACTED]"; }
    }

    private final HttpClient client;
    private final ObjectMapper mapper;
    private final URI humanBaseUri;
    private final URI agentEndpoint;
    private final Duration timeout;
    private final Credential credential;

    public HttpProductionTransport(HttpClient client, ObjectMapper mapper, URI humanBaseUri,
                                   URI agentEndpoint, Duration timeout, Credential credential) {
        this.client = Objects.requireNonNull(client, "client");
        this.mapper = Objects.requireNonNull(mapper, "mapper");
        this.humanBaseUri = requireHttpUri(humanBaseUri);
        this.agentEndpoint = requireHttpUri(agentEndpoint);
        this.timeout = Objects.requireNonNull(timeout, "timeout");
        if (timeout.isZero() || timeout.isNegative()) throw PlatformSdkException.invalidArgument();
        this.credential = credential;
    }

    public static HttpProductionTransport create(URI humanBaseUri, URI agentEndpoint, Credential credential) {
        return new HttpProductionTransport(HttpClient.newBuilder().version(HttpClient.Version.HTTP_2).build(),
            new ObjectMapper(), humanBaseUri, agentEndpoint, Duration.ofSeconds(30), credential);
    }

    @Override
    public <T> CompletionStage<T> call(Call call, JavaType responseType) {
        Objects.requireNonNull(call, "call");
        Objects.requireNonNull(responseType, "responseType");
        OperationCatalog.requireKnown(call.plane(), call.operation());
        HttpRequest request;
        try {
            request = call.plane() == OperationCatalog.Plane.HUMAN ? humanRequest(call) : agentRequest(call);
        } catch (IOException error) {
            return CompletableFuture.failedFuture(new PlatformSdkException(
                PlatformSdkException.Code.INVALID_ARGUMENT, PlatformSdkException.Retry.NEVER, null, null, null));
        }
        return client.sendAsync(request, HttpResponse.BodyHandlers.ofInputStream())
            .handle((response, failure) -> {
                if (failure != null) throw new CompletionException(new PlatformSdkException(
                    PlatformSdkException.Code.TRANSPORT_FAILURE, PlatformSdkException.Retry.SAFE, null, null, null));
                try (var body = response.body()) {
                    byte[] encoded = body.readNBytes(MAXIMUM_RESPONSE_BYTES + 1);
                    if (encoded.length > MAXIMUM_RESPONSE_BYTES) throw new PlatformSdkException(
                        PlatformSdkException.Code.DECODE_FAILURE, PlatformSdkException.Retry.NEVER, null, null, null);
                    return decode(response.statusCode(), encoded, responseType);
                } catch (IOException error) {
                    throw new CompletionException(new PlatformSdkException(PlatformSdkException.Code.TRANSPORT_FAILURE,
                        PlatformSdkException.Retry.SAFE, null, null, null));
                }
            });
    }

    private HttpRequest humanRequest(Call call) throws IOException {
        var route = OperationCatalog.HUMAN_ROUTES.get(call.operation());
        String path = route.path();
        for (String parameter : route.pathParameters()) {
            String value = call.pathParameters().get(parameter);
            if (value == null || value.isEmpty()) throw PlatformSdkException.invalidArgument();
            path = path.replace("{" + parameter + "}", encodePath(value));
        }
        var builder = common(endpoint(humanBaseUri, path), call);
        if (route.bodyless()) builder.method(route.method(), HttpRequest.BodyPublishers.noBody());
        else builder.method(route.method(), jsonBody(call.request()));
        return builder.build();
    }

    private HttpRequest agentRequest(Call call) throws IOException {
        var body = mapper.createObjectNode();
        body.put("operation", call.operation());
        body.set("request", mapper.valueToTree(call.request()));
        return common(agentEndpoint, call).POST(jsonBody(body)).build();
    }

    private HttpRequest.Builder common(URI uri, Call call) {
        var builder = HttpRequest.newBuilder(uri).timeout(timeout)
            .header("Accept", "application/json").header("Content-Type", "application/json")
            .header("User-Agent", "layerx-jvm/0.1.0");
        if (call.idempotencyKey() != null) builder.header("Idempotency-Key", call.idempotencyKey().value());
        if (credential != null) credential.apply(builder);
        return builder;
    }

    private HttpRequest.BodyPublisher jsonBody(Object value) throws IOException {
        return HttpRequest.BodyPublishers.ofByteArray(mapper.writeValueAsBytes(value == null ? Map.of() : value));
    }

    private <T> T decode(int status, byte[] encoded, JavaType type) {
        try {
            JsonNode envelope = mapper.readTree(encoded);
            if (envelope == null || !envelope.isObject() || !envelope.path("ok").isBoolean()) {
                throw new PlatformSdkException(PlatformSdkException.Code.DECODE_FAILURE,
                    PlatformSdkException.Retry.NEVER, null, null, null);
            }
            String trace = envelope.path("trace").isTextual() ? envelope.path("trace").textValue() : null;
            if (!envelope.path("ok").booleanValue()) throw serviceError(status, trace, envelope.path("error"));
            if (status < 200 || status >= 300 || envelope.has("error")) throw new PlatformSdkException(
                PlatformSdkException.Code.DECODE_FAILURE, PlatformSdkException.Retry.NEVER, trace, null, null);
            if (!envelope.has("result")) throw new PlatformSdkException(PlatformSdkException.Code.DECODE_FAILURE,
                PlatformSdkException.Retry.NEVER, trace, null, null);
            return mapper.convertValue(envelope.get("result"), type);
        } catch (PlatformSdkException error) {
            throw error;
        } catch (IOException | IllegalArgumentException error) {
            throw new PlatformSdkException(PlatformSdkException.Code.DECODE_FAILURE,
                PlatformSdkException.Retry.NEVER, null, null, null);
        }
    }

    private static PlatformSdkException serviceError(int status, String trace, JsonNode error) {
        String code = error.path("code").asText("");
        var mapped = switch (code) {
            case "rate-limited" -> PlatformSdkException.Code.RATE_LIMIT;
            case "conflict" -> PlatformSdkException.Code.IDEMPOTENCY_CONFLICT;
            case "refused-by-policy" -> PlatformSdkException.Code.POLICY_REFUSAL;
            case "refused-by-budget" -> PlatformSdkException.Code.BUDGET_REFUSAL;
            case "refused-by-capability", "forbidden" -> PlatformSdkException.Code.CAPABILITY_REFUSAL;
            case "refused-by-protocol", "refused-by-limit" -> PlatformSdkException.Code.CORE_REJECTION;
            case "unavailable", "upstream-degraded" -> PlatformSdkException.Code.UNAVAILABLE_CAPABILITY;
            default -> status >= 500 ? PlatformSdkException.Code.INTERNAL_FAULT : PlatformSdkException.Code.CORE_REJECTION;
        };
        var retry = switch (error.path("retry").asText("")) {
            case "retriable" -> PlatformSdkException.Retry.SAFE;
            case "retriable-after" -> PlatformSdkException.Retry.AFTER;
            case "structural", "final" -> PlatformSdkException.Retry.NEVER;
            default -> throw new PlatformSdkException(PlatformSdkException.Code.DECODE_FAILURE,
                PlatformSdkException.Retry.NEVER, trace, null, null);
        };
        Long after = error.path("retry_after_ms").canConvertToLong() ? error.path("retry_after_ms").longValue() : null;
        Integer resultCode = error.path("protocol_result_code").canConvertToInt()
            ? error.path("protocol_result_code").intValue() : null;
        return new PlatformSdkException(mapped, retry, trace, resultCode, after);
    }

    private static URI requireHttpUri(URI uri) {
        Objects.requireNonNull(uri, "uri");
        if (!("http".equalsIgnoreCase(uri.getScheme()) || "https".equalsIgnoreCase(uri.getScheme()))) {
            throw PlatformSdkException.invalidArgument();
        }
        if (uri.getHost() == null || uri.getUserInfo() != null || uri.getQuery() != null || uri.getFragment() != null) {
            throw PlatformSdkException.invalidArgument();
        }
        if ("http".equalsIgnoreCase(uri.getScheme()) && !isLoopback(uri.getHost())) {
            throw PlatformSdkException.invalidArgument();
        }
        return uri;
    }
    private static boolean isLoopback(String host) {
        String normalized = host.toLowerCase(java.util.Locale.ROOT);
        return normalized.equals("localhost") || normalized.equals("::1") || normalized.equals("[::1]")
            || normalized.startsWith("127.");
    }
    private static URI endpoint(URI base, String path) {
        String prefix = base.toString();
        while (prefix.endsWith("/")) prefix = prefix.substring(0, prefix.length() - 1);
        return URI.create(prefix + path);
    }
    private static String encodePath(String value) { return URLEncoder.encode(value, StandardCharsets.UTF_8).replace("+", "%20"); }
}
