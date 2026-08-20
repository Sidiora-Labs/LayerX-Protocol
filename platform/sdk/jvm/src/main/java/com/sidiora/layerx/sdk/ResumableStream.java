package com.sidiora.layerx.sdk;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.CompletionStage;
import java.util.concurrent.Flow;
import java.util.concurrent.SubmissionPublisher;

/** Cursor-checked stream state with atomic page acceptance and virtual-thread-friendly async I/O. */
public final class ResumableStream<T> {
    public record Cursor(String value) {
        public Cursor {
            if (value == null || value.isEmpty() || value.length() > 512 || value.indexOf('\0') >= 0) {
                throw PlatformSdkException.invalidArgument();
            }
        }
        @Override public String toString() { return value; }
    }
    public record Event<T>(String eventId, Cursor previousCursor, Cursor cursor, T value) {}
    public record Page<T>(Cursor requestedCursor, List<Event<T>> events, Cursor nextCursor) {
        public Page {
            Objects.requireNonNull(requestedCursor, "requestedCursor");
            events = List.copyOf(events);
            Objects.requireNonNull(nextCursor, "nextCursor");
        }
    }
    @FunctionalInterface public interface PageSource<T> { CompletionStage<Page<T>> fetch(Cursor cursor); }

    private Cursor cursor;
    private final Set<String> seen = new HashSet<>();

    public ResumableStream(Cursor cursor) { this.cursor = Objects.requireNonNull(cursor, "cursor"); }
    public synchronized Cursor cursor() { return cursor; }

    public synchronized List<Event<T>> accept(Page<T> page) {
        Objects.requireNonNull(page, "page");
        if (!page.requestedCursor().equals(cursor)) throw decodeFailure();
        Cursor expected = cursor;
        Set<String> pageIds = new HashSet<>();
        for (Event<T> event : page.events()) {
            if (event == null || event.eventId() == null || event.eventId().isEmpty()
                    || event.previousCursor() == null || event.cursor() == null
                    || !event.previousCursor().equals(expected)
                    || seen.contains(event.eventId()) || !pageIds.add(event.eventId())) throw decodeFailure();
            expected = event.cursor();
        }
        if (!page.nextCursor().equals(expected)) throw decodeFailure();
        seen.addAll(pageIds);
        cursor = page.nextCursor();
        return List.copyOf(new ArrayList<>(page.events()));
    }

    public Flow.Publisher<Event<T>> publisher(PageSource<T> source) {
        Objects.requireNonNull(source, "source");
        return subscriber -> {
            var output = new SubmissionPublisher<Event<T>>();
            output.subscribe(subscriber);
            Thread.ofVirtual().name("layerx-stream").start(() -> {
                try {
                    while (output.hasSubscribers()) {
                        Page<T> page = source.fetch(cursor()).toCompletableFuture().join();
                        for (Event<T> event : accept(page)) output.submit(event);
                    }
                    output.close();
                } catch (Throwable failure) {
                    output.closeExceptionally(failure);
                }
            });
        };
    }

    private static PlatformSdkException decodeFailure() {
        return new PlatformSdkException(PlatformSdkException.Code.DECODE_FAILURE,
            PlatformSdkException.Retry.NEVER, null, null, null);
    }
}
