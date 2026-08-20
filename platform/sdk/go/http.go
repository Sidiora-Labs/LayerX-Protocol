package layerx

import (
	"bytes"
	"context"
	"encoding/json"
	"io"
	"net"
	"net/http"
	"net/url"
	"strings"
)

const maximumHTTPResponseBytes = 8 * 1024 * 1024

type RequestAuthorizer func(*http.Request) error

type HumanHTTPTransport struct {
	baseURL    *url.URL
	client     *http.Client
	authorizer RequestAuthorizer
}

func NewHumanHTTPTransport(baseURL string, client *http.Client, authorizer RequestAuthorizer) (*HumanHTTPTransport, error) {
	parsed, err := url.Parse(baseURL)
	if err != nil || parsed.Host == "" || parsed.User != nil || (parsed.Scheme != "https" && parsed.Scheme != "http") {
		return nil, newSDKError(ErrorInvalidArgument, RetryNever)
	}
	if parsed.Scheme == "http" && !loopbackHost(parsed.Hostname()) {
		return nil, newSDKError(ErrorInvalidArgument, RetryNever)
	}
	if client == nil {
		client = http.DefaultClient
	}
	return &HumanHTTPTransport{baseURL: parsed, client: client, authorizer: authorizer}, nil
}

func loopbackHost(host string) bool {
	if host == "localhost" {
		return true
	}
	address := net.ParseIP(host)
	return address != nil && address.IsLoopback()
}

func (transport *HumanHTTPTransport) Call(ctx context.Context, call TransportCall) (json.RawMessage, error) {
	if transport == nil || call.Plane != PlaneHuman {
		return nil, newSDKError(ErrorUnavailableCapability, RetryNever)
	}
	operation := HumanOperation(call.Operation)
	metadata, ok := operation.Metadata()
	if !ok {
		return nil, newSDKError(ErrorInvalidArgument, RetryNever)
	}
	path := metadata.Path
	for name, value := range call.PathParameters {
		if name == "" || value == "" {
			return nil, newSDKError(ErrorInvalidArgument, RetryNever)
		}
		path = strings.ReplaceAll(path, "{"+name+"}", url.PathEscape(value))
	}
	if strings.ContainsAny(path, "{}") {
		return nil, newSDKError(ErrorInvalidArgument, RetryNever)
	}
	target := *transport.baseURL
	target.Path = strings.TrimRight(target.Path, "/") + path
	target.RawQuery = cloneValues(call.Query).Encode()

	var body io.Reader
	if metadata.Request != "Empty" {
		body = bytes.NewReader(call.Request)
	}
	request, err := http.NewRequestWithContext(ctx, metadata.Method, target.String(), body)
	if err != nil {
		return nil, newSDKError(ErrorInvalidArgument, RetryNever)
	}
	request.Header.Set("Accept", "application/json")
	request.Header.Set("User-Agent", "layerx-go/0.1.0")
	if body != nil {
		request.Header.Set("Content-Type", "application/json")
	}
	if call.IdempotencyKey.valid() {
		request.Header.Set("Idempotency-Key", call.IdempotencyKey.String())
	}
	if transport.authorizer != nil {
		if err := transport.authorizer(request); err != nil {
			return nil, transportError(ctx, err)
		}
	}
	response, err := transport.client.Do(request)
	if err != nil {
		return nil, transportError(ctx, err)
	}
	defer response.Body.Close()
	limited := io.LimitReader(response.Body, maximumHTTPResponseBytes+1)
	encoded, err := io.ReadAll(limited)
	if err != nil {
		return nil, transportError(ctx, err)
	}
	if len(encoded) > maximumHTTPResponseBytes {
		return nil, newSDKError(ErrorDecodeFailure, RetryNever)
	}
	var envelope humanEnvelope
	if err := json.Unmarshal(encoded, &envelope); err != nil || envelope.Trace == "" {
		return nil, newSDKError(ErrorDecodeFailure, RetryNever)
	}
	if envelope.OK {
		if len(envelope.Result) == 0 || envelope.Error != nil || response.StatusCode < 200 || response.StatusCode >= 300 {
			return nil, newSDKError(ErrorDecodeFailure, RetryNever)
		}
		return append(json.RawMessage(nil), envelope.Result...), nil
	}
	if envelope.Error == nil || envelope.Error.Code == "" || response.StatusCode >= 200 && response.StatusCode < 300 {
		return nil, newSDKError(ErrorDecodeFailure, RetryNever)
	}
	return nil, envelope.Error.sdkError(envelope.Trace)
}

type humanEnvelope struct {
	OK     bool            `json:"ok"`
	Result json.RawMessage `json:"result"`
	Error  *humanAPIError  `json:"error"`
	Trace  string          `json:"trace"`
}

type humanAPIError struct {
	Code             HumanErrorCode `json:"code"`
	Retry            string         `json:"retry"`
	RetryAfterMillis *uint64        `json:"retry_after_ms,omitempty"`
}

func (apiError *humanAPIError) sdkError(trace string) *SDKError {
	code := ErrorCoreRejection
	switch apiError.Code {
	case HumanErrorRateLimited:
		code = ErrorRateLimit
	case HumanErrorUnavailable, HumanErrorUpstreamDegraded:
		code = ErrorTransportFailure
	case HumanErrorRefusedByPolicy:
		code = ErrorPolicyRefusal
	case HumanErrorRefusedByBudget, HumanErrorRefusedByLimit:
		code = ErrorBudgetRefusal
	case HumanErrorRefusedByCapability, HumanErrorForbidden, HumanErrorUnauthenticated, HumanErrorSessionExpired, HumanErrorStepUpRequired:
		code = ErrorCapabilityRefusal
	case HumanErrorRefusedByProtocol:
		code = ErrorCoreRejection
	case HumanErrorConflict:
		code = ErrorIdempotencyConflict
	}
	retry := RetryNever
	switch apiError.Retry {
	case "retriable":
		retry = RetrySafe
	case "retriable-after":
		retry = RetryAfter
	case "structural", "final":
		retry = RetryNever
	default:
		return newSDKError(ErrorDecodeFailure, RetryNever)
	}
	result := newSDKError(code, retry)
	result.ServiceCode = string(apiError.Code)
	result.RequestID = trace
	result.RetryAfterMillis = apiError.RetryAfterMillis
	return result
}
