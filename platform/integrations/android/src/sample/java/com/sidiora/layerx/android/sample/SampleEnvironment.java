package com.sidiora.layerx.android.sample;

import com.sidiora.layerx.android.MobileIntegrationException;
import com.sidiora.layerx.android.PublishableConfiguration;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/** Declared-key sources for the sample: the process environment or Android string resources. */
public final class SampleEnvironment {
    private SampleEnvironment() {}

    public static PublishableConfiguration configuration(Map<String, String> named) {
        Map<String, String> declared = new HashMap<>();
        for (Map.Entry<String, String> entry : named.entrySet()) {
            if (entry.getKey() == null || entry.getValue() == null || entry.getValue().isEmpty()) continue;
            String key = PublishableConfiguration.declaredKeyForEnvironmentVariable(
                entry.getKey().toUpperCase(Locale.ROOT));
            if (key != null) declared.put(key, entry.getValue());
        }
        if (declared.isEmpty()) {
            throw MobileIntegrationException.of(MobileIntegrationException.Code.INVALID_CONFIGURATION);
        }
        return PublishableConfiguration.of(declared);
    }

    public static String required(Map<String, String> environment, String name) {
        String value = environment.get(name);
        if (value == null || value.isEmpty()) {
            throw MobileIntegrationException.of(MobileIntegrationException.Code.INVALID_CONFIGURATION);
        }
        return value;
    }
}
