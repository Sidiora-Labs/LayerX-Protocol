package com.sidiora.layerx.sdk;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonValue;
import java.math.BigInteger;
import java.util.Objects;
import java.util.regex.Pattern;

/** An unsigned, integer-only 128-bit amount expressed in protocol base units. */
public record ProtocolAmount(BigInteger value) implements Comparable<ProtocolAmount> {
    public static final BigInteger MAX_VALUE = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);
    private static final Pattern CANONICAL = Pattern.compile("0|[1-9][0-9]*");

    public ProtocolAmount {
        Objects.requireNonNull(value, "value");
        if (value.signum() < 0 || value.compareTo(MAX_VALUE) > 0) {
            throw PlatformSdkException.invalidArgument();
        }
    }

    @JsonCreator
    public static ProtocolAmount parse(String value) {
        if (value == null || !CANONICAL.matcher(value).matches()) {
            throw PlatformSdkException.invalidArgument();
        }
        return new ProtocolAmount(new BigInteger(value));
    }

    public static ProtocolAmount of(BigInteger value) {
        return new ProtocolAmount(value);
    }

    @Override @JsonValue
    public String toString() {
        return value.toString(10);
    }

    @Override
    public int compareTo(ProtocolAmount other) {
        return value.compareTo(other.value);
    }
}
