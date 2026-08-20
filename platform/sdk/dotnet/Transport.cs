#nullable enable

using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace LayerX.Sdk;

public sealed class AccessToken : IDisposable
{
    private static readonly UTF8Encoding StrictUtf8 = new(false, true);
    private readonly SecretBytes _secret;

    public AccessToken(ReadOnlySpan<byte> bytes)
    {
        try { _ = StrictUtf8.GetCharCount(bytes); }
        catch (DecoderFallbackException) { throw new PlatformSdkException(SdkErrorCode.InvalidArgument, RetryClass.Never); }
        _secret = new SecretBytes(bytes);
    }

    internal void Authorize(HttpRequestMessage request) => _secret.Use(bytes =>
    {
        var value = StrictUtf8.GetString(bytes.Span);
        if (value.Contains('\r') || value.Contains('\n'))
            throw new PlatformSdkException(SdkErrorCode.InvalidArgument, RetryClass.Never);
        request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", value);
        return true;
    });

    public void Dispose() => _secret.Dispose();
    public override string ToString() => "[REDACTED]";
}

public sealed class HumanHttpTransport : IPlatformTransport
{
    private const int MaximumResponseBytes = 8 * 1024 * 1024;
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web);
    private readonly Uri _baseUri;
    private readonly HttpClient _httpClient;
    private readonly AccessToken? _accessToken;

    public HumanHttpTransport(Uri baseUri, HttpClient? httpClient = null, AccessToken? accessToken = null)
    {
        if (!baseUri.IsAbsoluteUri || !string.IsNullOrEmpty(baseUri.UserInfo) || string.IsNullOrEmpty(baseUri.Host) ||
            (baseUri.Scheme != Uri.UriSchemeHttps && (baseUri.Scheme != Uri.UriSchemeHttp || !IsLoopback(baseUri.Host))))
            throw new PlatformSdkException(SdkErrorCode.InvalidArgument, RetryClass.Never);
        _baseUri = baseUri;
        _httpClient = httpClient ?? new HttpClient();
        _accessToken = accessToken;
    }

    public async Task<JsonValue> SendAsync(TransportCall call, CancellationToken cancellationToken = default)
    {
        var descriptor = call.Operation.Descriptor();
        if (descriptor.Plane != PlatformPlane.Human)
            throw new PlatformSdkException(SdkErrorCode.UnavailableCapability, RetryClass.Never);
        var path = ResolvePath(descriptor.Path, call.PathParameters);
        var target = new Uri(_baseUri, path);
        if (target.Scheme != _baseUri.Scheme || target.Host != _baseUri.Host || target.Port != _baseUri.Port)
            throw new PlatformSdkException(SdkErrorCode.InvalidArgument, RetryClass.Never);

        using var request = new HttpRequestMessage(ToHttpMethod(descriptor.Method), target);
        request.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/json"));
        request.Headers.UserAgent.ParseAdd("layerx-dotnet/0.1.0");
        if (!descriptor.Bodyless)
        {
            var encoded = JsonSerializer.SerializeToUtf8Bytes(call.Request, JsonOptions);
            request.Content = new ByteArrayContent(encoded);
            request.Content.Headers.ContentType = new MediaTypeHeaderValue("application/json");
        }
        if (call.IdempotencyKey is { } key)
            request.Headers.TryAddWithoutValidation("Idempotency-Key", key.Value);
        _accessToken?.Authorize(request);

        using var response = await _httpClient.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, cancellationToken).ConfigureAwait(false);
        var encodedResponse = await ReadBoundedAsync(response.Content, cancellationToken).ConfigureAwait(false);
        HumanEnvelope? envelope;
        try { envelope = JsonSerializer.Deserialize<HumanEnvelope>(encodedResponse, JsonOptions); }
        catch (JsonException) { throw new PlatformSdkException(SdkErrorCode.DecodeFailure, RetryClass.Never); }
        if (envelope is null || string.IsNullOrEmpty(envelope.Trace) || Encoding.UTF8.GetByteCount(envelope.Trace) > 512 ||
            envelope.Trace.Contains('\0') || envelope.Trace.Contains('\r') || envelope.Trace.Contains('\n'))
            throw new PlatformSdkException(SdkErrorCode.DecodeFailure, RetryClass.Never);
        if (envelope.Ok)
        {
            if (!response.IsSuccessStatusCode || envelope.Error is not null || envelope.Result is null)
                throw new PlatformSdkException(SdkErrorCode.DecodeFailure, RetryClass.Never);
            return envelope.Result;
        }
        if (response.IsSuccessStatusCode || envelope.Error is null || envelope.Result is not null)
            throw new PlatformSdkException(SdkErrorCode.DecodeFailure, RetryClass.Never);
        throw envelope.Error.ToSdkException(envelope.Trace);
    }

    private static async Task<byte[]> ReadBoundedAsync(HttpContent content, CancellationToken cancellationToken)
    {
        await using var stream = await content.ReadAsStreamAsync(cancellationToken).ConfigureAwait(false);
        using var output = new MemoryStream();
        var buffer = new byte[16 * 1024];
        while (true)
        {
            var count = await stream.ReadAsync(buffer.AsMemory(), cancellationToken).ConfigureAwait(false);
            if (count == 0) return output.ToArray();
            if (output.Length + count > MaximumResponseBytes)
                throw new PlatformSdkException(SdkErrorCode.DecodeFailure, RetryClass.Never);
            output.Write(buffer, 0, count);
        }
    }

    private static string ResolvePath(string template, IReadOnlyDictionary<string, string> parameters)
    {
        var path = template;
        foreach (var (name, value) in parameters)
        {
            var token = "{" + name + "}";
            if (string.IsNullOrEmpty(name) || string.IsNullOrEmpty(value) || !path.Contains(token, StringComparison.Ordinal))
                throw new PlatformSdkException(SdkErrorCode.InvalidArgument, RetryClass.Never);
            path = path.Replace(token, Uri.EscapeDataString(value), StringComparison.Ordinal);
        }
        if (!path.StartsWith("/", StringComparison.Ordinal) || path.Contains('{') || path.Contains('}'))
            throw new PlatformSdkException(SdkErrorCode.InvalidArgument, RetryClass.Never);
        return path;
    }

    private static HttpMethod ToHttpMethod(SdkHttpMethod method) => method switch
    {
        SdkHttpMethod.Get => HttpMethod.Get,
        SdkHttpMethod.Post => HttpMethod.Post,
        SdkHttpMethod.Put => HttpMethod.Put,
        SdkHttpMethod.Patch => HttpMethod.Patch,
        SdkHttpMethod.Delete => HttpMethod.Delete,
        _ => throw new PlatformSdkException(SdkErrorCode.InvalidArgument, RetryClass.Never),
    };

    private static bool IsLoopback(string host)
    {
        if (string.Equals(host, "localhost", StringComparison.OrdinalIgnoreCase)) return true;
        return IPAddress.TryParse(host, out var address) && IPAddress.IsLoopback(address);
    }

    private sealed record HumanEnvelope(
        [property: JsonPropertyName("ok")] bool Ok,
        [property: JsonPropertyName("result")] JsonValue? Result,
        [property: JsonPropertyName("error")] HumanApiError? Error,
        [property: JsonPropertyName("trace")] string Trace);

    private sealed record HumanApiError(
        [property: JsonPropertyName("code")] string Code,
        [property: JsonPropertyName("retry")] string Retry,
        [property: JsonPropertyName("retry_after_ms")] ulong? RetryAfterMilliseconds)
    {
        public PlatformSdkException ToSdkException(string trace)
        {
            var code = Code switch
            {
                "rate-limited" => SdkErrorCode.RateLimit,
                "unavailable" or "upstream-degraded" => SdkErrorCode.TransportFailure,
                "refused-by-policy" => SdkErrorCode.PolicyRefusal,
                "refused-by-budget" or "refused-by-limit" => SdkErrorCode.BudgetRefusal,
                "refused-by-capability" or "forbidden" or "unauthenticated" or "session-expired" or "step-up-required" => SdkErrorCode.CapabilityRefusal,
                "conflict" => SdkErrorCode.IdempotencyConflict,
                _ => SdkErrorCode.CoreRejection,
            };
            var retry = Retry switch
            {
                "retriable" => RetryClass.Safe,
                "retriable-after" when RetryAfterMilliseconds is not null => RetryClass.After,
                "structural" or "final" => RetryClass.Never,
                _ => throw new PlatformSdkException(SdkErrorCode.DecodeFailure, RetryClass.Never),
            };
            return new PlatformSdkException(code, retry, trace, retryAfterMilliseconds: RetryAfterMilliseconds);
        }
    }
}
