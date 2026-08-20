package com.sidiora.layerx.android.sample;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ObjectNode;
import com.sidiora.layerx.android.LayerXAndroid;
import com.sidiora.layerx.android.MobileIntegrationException;
import com.sidiora.layerx.android.ReceiptGate;
import com.sidiora.layerx.android.VerifiedEventConsumer;
import com.sidiora.layerx.sdk.IdempotencyKey;
import com.sidiora.layerx.sdk.PlatformSdkException;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletionException;

/** Application state driven entirely by verified service answers. */
public final class WalletModel {
    public record Snapshot(String serviceVersion, String displayName, int activityCount,
                           ReceiptGate.State settlement, List<String> deliveries, String refusal) {
        public Snapshot { deliveries = List.copyOf(deliveries); }
        public static Snapshot empty() { return new Snapshot("", "", 0, null, List.of(), null); }
    }

    @FunctionalInterface public interface Backoff {
        void pause(long millis) throws InterruptedException;
    }

    private final LayerXAndroid mobile;
    private final ReceiptGate gate;
    private final ObjectMapper mapper;
    private final List<String> deliveries = new ArrayList<>();
    private Snapshot snapshot = Snapshot.empty();

    public WalletModel(LayerXAndroid mobile, ReceiptGate.ReceiptResolver receipts) {
        this.mobile = Objects.requireNonNull(mobile, "mobile");
        this.gate = mobile.gate(Objects.requireNonNull(receipts, "receipts"));
        this.mapper = mobile.mapper();
    }

    public synchronized Snapshot current() { return snapshot; }

    public synchronized Snapshot refresh() {
        try {
            JsonNode version = await(mobile.client().version());
            JsonNode profile = await(mobile.client().profile());
            ObjectNode query = mapper.createObjectNode();
            query.put("page_limit", 25);
            JsonNode activity = await(mobile.client().activity(query));
            snapshot = new Snapshot(
                text(version, "version", "protocol_version"),
                text(profile, "display_name", "email"),
                entries(activity),
                snapshot.settlement(),
                deliveries,
                null);
        } catch (RuntimeException error) {
            snapshot = new Snapshot(snapshot.serviceVersion(), snapshot.displayName(), snapshot.activityCount(),
                snapshot.settlement(), deliveries, refusal(error));
        }
        return snapshot;
    }

    public synchronized Snapshot pay(JsonNode quoteRequest, ReceiptGate.Expectation expectation, IdempotencyKey key) {
        try {
            JsonNode quote = await(mobile.client().quote(quoteRequest));
            if (!quote.path("quote_id").isTextual()) {
                throw MobileIntegrationException.of(MobileIntegrationException.Code.DECODE_FAILURE);
            }
            ObjectNode commit = mapper.createObjectNode();
            commit.put("quote_id", quote.path("quote_id").textValue());
            JsonNode journey = await(mobile.client().commit(commit, key));
            snapshot = new Snapshot(snapshot.serviceVersion(), snapshot.displayName(), snapshot.activityCount(),
                gate.project(journey, expectation), deliveries, null);
        } catch (RuntimeException error) {
            snapshot = new Snapshot(snapshot.serviceVersion(), snapshot.displayName(), snapshot.activityCount(),
                null, deliveries, refusal(error));
        }
        return snapshot;
    }

    public synchronized Snapshot awaitSettlement(String journeyId, ReceiptGate.Expectation expectation,
                                                 int attempts, Backoff backoff) {
        for (int attempt = 0; attempt < Math.max(attempts, 1); attempt++) {
            try {
                JsonNode journey = await(mobile.client().journey(journeyId));
                ReceiptGate.State state = gate.project(journey, expectation);
                snapshot = new Snapshot(snapshot.serviceVersion(), snapshot.displayName(), snapshot.activityCount(),
                    state, deliveries, null);
                if (!(state instanceof ReceiptGate.Pending)) return snapshot;
                backoff.pause(Math.min(attempt + 1, 10) * 250L);
            } catch (InterruptedException interrupted) {
                Thread.currentThread().interrupt();
                return snapshot;
            } catch (RuntimeException error) {
                snapshot = new Snapshot(snapshot.serviceVersion(), snapshot.displayName(), snapshot.activityCount(),
                    snapshot.settlement(), deliveries, refusal(error));
                return snapshot;
            }
        }
        return snapshot;
    }

    public synchronized Snapshot deliver(byte[] rawBody, Map<String, String> headerFields) {
        try {
            VerifiedEventConsumer.Outcome outcome = mobile.consume(rawBody, headerFields, this::record);
            if (outcome != VerifiedEventConsumer.Outcome.PROCESSED) {
                append(outcome.name().toLowerCase(java.util.Locale.ROOT));
            }
            snapshot = new Snapshot(snapshot.serviceVersion(), snapshot.displayName(), snapshot.activityCount(),
                snapshot.settlement(), deliveries, null);
        } catch (RuntimeException error) {
            snapshot = new Snapshot(snapshot.serviceVersion(), snapshot.displayName(), snapshot.activityCount(),
                snapshot.settlement(), deliveries, refusal(error));
        }
        return snapshot;
    }

    private void record(JsonNode event, String deliveryId) {
        String kind = text(event, "type", "kind");
        append(deliveryId + ":" + (kind.isEmpty() ? "event" : kind));
    }

    private void append(String entry) {
        deliveries.add(entry);
        while (deliveries.size() > 64) deliveries.remove(0);
    }

    private static <T> T await(java.util.concurrent.CompletionStage<T> stage) {
        try {
            return stage.toCompletableFuture().join();
        } catch (CompletionException error) {
            Throwable cause = error.getCause();
            if (cause instanceof RuntimeException runtime) throw runtime;
            throw error;
        }
    }

    private static String text(JsonNode value, String primary, String fallback) {
        if (value.path(primary).isTextual()) return value.path(primary).textValue();
        if (value.path(fallback).isTextual()) return value.path(fallback).textValue();
        return "";
    }

    private static int entries(JsonNode value) {
        return value.path("entries").isArray() ? value.path("entries").size() : 0;
    }

    private static String refusal(RuntimeException error) {
        if (error instanceof MobileIntegrationException mobile) return mobile.code().wire();
        if (error instanceof PlatformSdkException sdk) return sdk.code().wire();
        return "transport-failure";
    }
}
