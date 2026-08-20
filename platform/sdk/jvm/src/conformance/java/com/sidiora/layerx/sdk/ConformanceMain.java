package com.sidiora.layerx.sdk;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.HashSet;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;

/** Schema-golden conformance gate, compiled only by the conformance Maven profile. */
public final class ConformanceMain {
    private static final ObjectMapper JSON = new ObjectMapper();
    private ConformanceMain() {}

    public static void main(String[] arguments) throws Exception {
        Path root = arguments.length == 0 ? Path.of("../../..") : Path.of(arguments[0]);
        Path golden = root.resolve("human/schema/human-api/golden");
        for (Map.Entry<String, OperationCatalog.Route> operation : OperationCatalog.HUMAN_ROUTES.entrySet()) {
            verifyTriplet(golden, operation.getKey(), operation.getValue());
        }
        Set<String> sdkCodes = new HashSet<>();
        for (PlatformSdkException.Code code : PlatformSdkException.Code.values()) {
            if (!sdkCodes.add(code.wire())) throw new IllegalStateException("duplicate SDK error code");
        }
        System.out.printf("agent_operations=%d human_operations=%d human_golden_triplets=%d sdk_error_codes=%d%n",
            OperationCatalog.AGENT_OPERATIONS.size(), OperationCatalog.HUMAN_ROUTES.size(),
            OperationCatalog.HUMAN_ROUTES.size(), sdkCodes.size());
    }

    private static void verifyTriplet(Path golden, String operation, OperationCatalog.Route route) throws IOException {
        Path prefix = golden.resolve(operation);
        JsonNode request = read(prefix.resolveSibling(prefix.getFileName() + ".request.json"));
        if (!route.method().equals(text(request, "method")) || !pathMatches(route.path(), text(request, "path"))) {
            throw new IllegalStateException(operation + " request method/path diverges from schema");
        }
        JsonNode headers = request.path("headers");
        if (route.idempotency() && (!headers.isObject() || headers.path("Idempotency-Key").asText().isEmpty())) {
            throw new IllegalStateException(operation + " request omits Idempotency-Key");
        }
        rejectNumericMoney(request.path("body"), "body");
        verifySuccess(operation, read(prefix.resolveSibling(prefix.getFileName() + ".response.json")));
        verifyFailure(operation, read(prefix.resolveSibling(prefix.getFileName() + ".failure.json")));
    }

    private static void verifySuccess(String operation, JsonNode vector) {
        int status = vector.path("status").asInt(-1);
        JsonNode body = vector.path("body");
        if (status < 200 || status >= 300 || !body.path("ok").asBoolean(false)
                || body.path("trace").asText().isEmpty() || !body.has("result") || body.has("error")) {
            throw new IllegalStateException(operation + " success vector is not a typed success envelope");
        }
        rejectNumericMoney(body.path("result"), "result");
    }

    private static void verifyFailure(String operation, JsonNode vector) {
        int status = vector.path("status").asInt(-1);
        JsonNode body = vector.path("body"), error = body.path("error");
        String retry = error.path("retry").asText();
        if ((status >= 200 && status < 300) || body.path("ok").asBoolean(true)
                || body.path("trace").asText().isEmpty()
                || !OperationCatalog.HUMAN_ERROR_CODES.contains(error.path("code").asText())
                || !(retry.equals("retriable") || retry.equals("retriable-after")
                    || retry.equals("structural") || retry.equals("final"))) {
            throw new IllegalStateException(operation + " failure vector is not a typed failure envelope");
        }
    }

    private static void rejectNumericMoney(JsonNode node, String name) {
        if (node.isObject()) node.fields().forEachRemaining(field -> rejectNumericMoney(field.getValue(), field.getKey()));
        else if (node.isArray()) node.forEach(child -> rejectNumericMoney(child, name));
        else if ((name.equals("amount") || name.equals("amounts")) && node.isNumber()) {
            throw new IllegalStateException(name + " is encoded as a JSON number");
        }
    }

    private static boolean pathMatches(String template, String actual) {
        StringBuilder expression = new StringBuilder("^");
        int position = 0;
        var parameter = Pattern.compile("\\{[^}]+}").matcher(template);
        while (parameter.find()) {
            expression.append(Pattern.quote(template.substring(position, parameter.start()))).append("[^/]+");
            position = parameter.end();
        }
        expression.append(Pattern.quote(template.substring(position))).append('$');
        return actual.matches(expression.toString());
    }
    private static String text(JsonNode node, String field) {
        JsonNode value = node.path(field);
        if (!value.isTextual()) throw new IllegalStateException(field + " is missing");
        return value.textValue();
    }
    private static JsonNode read(Path path) throws IOException { return JSON.readTree(Files.readAllBytes(path)); }
}
