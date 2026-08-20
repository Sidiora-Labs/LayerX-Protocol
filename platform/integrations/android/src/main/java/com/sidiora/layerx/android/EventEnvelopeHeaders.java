package com.sidiora.layerx.android;

import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;

/** The signed-delivery headers a relayed LayerX event must carry to be accepted on device. */
public record EventEnvelopeHeaders(String id, String timestamp, String keyId, String signature) {
    public static final String ID_HEADER = "LayerX-Delivery-Id";
    public static final String TIMESTAMP_HEADER = "LayerX-Timestamp";
    public static final String KEY_ID_HEADER = "LayerX-Key-Id";
    public static final String SIGNATURE_HEADER = "LayerX-Signature";

    public EventEnvelopeHeaders {
        Objects.requireNonNull(id, "id");
        Objects.requireNonNull(timestamp, "timestamp");
        Objects.requireNonNull(keyId, "keyId");
        Objects.requireNonNull(signature, "signature");
    }

    public static EventEnvelopeHeaders of(Map<String, String> fields) {
        Map<String, String> normalized = new LinkedHashMap<>();
        for (Map.Entry<String, String> field : fields.entrySet()) {
            if (field.getKey() == null || field.getValue() == null) continue;
            normalized.put(field.getKey().toLowerCase(Locale.ROOT), field.getValue());
        }
        String id = normalized.get(ID_HEADER.toLowerCase(Locale.ROOT));
        String timestamp = normalized.get(TIMESTAMP_HEADER.toLowerCase(Locale.ROOT));
        String keyId = normalized.get(KEY_ID_HEADER.toLowerCase(Locale.ROOT));
        String signature = normalized.get(SIGNATURE_HEADER.toLowerCase(Locale.ROOT));
        if (id == null || timestamp == null || keyId == null || signature == null) {
            throw MobileIntegrationException.of(MobileIntegrationException.Code.INVALID_EVENT);
        }
        return new EventEnvelopeHeaders(id, timestamp, keyId, signature);
    }
}
