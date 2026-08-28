package layerx

import (
	"context"
	"errors"
)

const (
	MaximumProgramCalldataBytes = 1_048_576
	MaximumProgramCapabilities = 256
	MaximumProgramCapabilityBytes = 4_096
)

type ProgramVersion struct { Number uint32 `json:"number"`; CodeHash [32]byte `json:"code_hash"`; ABIVersion uint16 `json:"abi_version"`; InterfaceDigest *[32]byte `json:"interface_digest,omitempty"` }
type ProgramFreshness struct { ObservedSequence uint64 `json:"observed_sequence"`; ObservedAt uint64 `json:"observed_at"`; ValidThrough uint64 `json:"valid_through"`; StateRoot [32]byte `json:"state_root"` }
type ProgramDiscovery struct { ProgramID [32]byte `json:"program_id"`; Lifecycle string `json:"lifecycle"`; ActiveVersion ProgramVersion `json:"active_version"`; Versions []ProgramVersion `json:"versions"`; ReceiptDigest [32]byte `json:"receipt_digest"`; Freshness ProgramFreshness `json:"freshness"` }
type ProgramInterface struct { ProgramID [32]byte `json:"program_id"`; Version uint32 `json:"version"`; CodeHash [32]byte `json:"code_hash"`; ABIVersion uint16 `json:"abi_version"`; Interface []byte `json:"interface,omitempty"`; InterfaceDigest *[32]byte `json:"interface_digest,omitempty"`; ReceiptDigest [32]byte `json:"receipt_digest"`; Freshness ProgramFreshness `json:"freshness"` }
type ProgramCall struct { ProgramID [32]byte `json:"program_id"`; Version uint32 `json:"version"`; CodeHash [32]byte `json:"code_hash"`; ABIVersion uint16 `json:"abi_version"`; Entrypoint string `json:"entrypoint"`; Calldata []byte `json:"calldata"`; Fuel uint64 `json:"fuel"`; FeeLimit Uint128 `json:"fee_limit"`; Capabilities [][]byte `json:"capabilities"`; SignedActivity []byte `json:"signed_activity"` }
type ProgramEvidence struct { Receipt []byte `json:"receipt"`; Authority AuthorizedBatch `json:"authority"`; ActivityID [32]byte `json:"activity_id"`; Outcome map[string]any `json:"outcome"`; TerminalAttachments [][]byte `json:"terminal_attachments"` }
type VerifiedProgramExecution struct { Verification VerifiedReceipt; Outcome map[string]any; TerminalAttachments [][]byte }
type ProgramSubmission struct { State string `json:"state"`; ActivityID [32]byte `json:"activity_id"`; IdempotencyKey [32]byte `json:"idempotency_key"`; RetainedSignedActivity []byte `json:"retained_signed_activity,omitempty"`; Evidence *ProgramEvidence `json:"evidence,omitempty"` }

func validateProgramCall(call ProgramCall) error {
	if call.Version == 0 || call.ABIVersion == 0 || len(call.Entrypoint) == 0 || len(call.Entrypoint) > 255 || len(call.Calldata) > MaximumProgramCalldataBytes || len(call.Capabilities) > MaximumProgramCapabilities || len(call.SignedActivity) == 0 { return errors.New("invalid program call") }
	for _, capability := range call.Capabilities { if len(capability) == 0 || len(capability) > MaximumProgramCapabilityBytes { return errors.New("invalid program capability") } }
	return nil
}

func verifyProgramEvidence(evidence ProgramEvidence, call ProgramCall) (VerifiedProgramExecution, error) {
	if err := validateProgramCall(call); err != nil { return VerifiedProgramExecution{}, err }
	verified, err := VerifyReceiptOutcome(evidence.Receipt, evidence.Authority); if err != nil { return VerifiedProgramExecution{}, err }
	if verified.Receipt.ActivityID != evidence.ActivityID || verified.Receipt.ModuleID != 9 || verified.Receipt.Operation != 3 || verified.Receipt.ModuleVersion != uint32(call.ABIVersion) { return VerifiedProgramExecution{}, errors.New("program receipt binding failed") }
	return VerifiedProgramExecution{Verification: verified, Outcome: evidence.Outcome, TerminalAttachments: evidence.TerminalAttachments}, nil
}

type Programs struct { client *Client }
func NewPrograms(client *Client) (*Programs, error) { if client == nil { return nil, errors.New("client required") }; return &Programs{client: client}, nil }
func (p *Programs) Discover(ctx context.Context, program [32]byte) (ProgramDiscovery, error) { var out ProgramDiscovery; err := p.client.call(ctx, PlaneAgent, "program.discover", false, map[string]any{"program_id": program, "requested_verification_level": "sequencer-signed"}, &out, CallOptions{}); return out, err }
func (p *Programs) Interface(ctx context.Context, program [32]byte, version uint32) (ProgramInterface, error) { var out ProgramInterface; err := p.client.call(ctx, PlaneAgent, "program.interface", false, map[string]any{"program_id": program, "version": version, "requested_verification_level": "sequencer-signed"}, &out, CallOptions{}); return out, err }
func (p *Programs) Simulate(ctx context.Context, call ProgramCall) (VerifiedProgramExecution, error) { if err := validateProgramCall(call); err != nil { return VerifiedProgramExecution{}, err }; var evidence ProgramEvidence; if err := p.client.call(ctx, PlaneAgent, "program.simulate", false, call, &evidence, CallOptions{}); err != nil { return VerifiedProgramExecution{}, err }; return verifyProgramEvidence(evidence, call) }
func (p *Programs) Submit(ctx context.Context, call ProgramCall, key IdempotencyKey) (ProgramSubmission, *VerifiedProgramExecution, error) { if err := validateProgramCall(call); err != nil { return ProgramSubmission{}, nil, err }; var out ProgramSubmission; if err := p.client.call(ctx, PlaneAgent, "program.call", true, call, &out, CallOptions{IdempotencyKey: key}); err != nil { return ProgramSubmission{}, nil, err }; if out.State != "executed" { return out, nil, nil }; if out.Evidence == nil { return ProgramSubmission{}, nil, errors.New("executed program response lacks evidence") }; verified, err := verifyProgramEvidence(*out.Evidence, call); return out, &verified, err }
func (p *Programs) Receipt(ctx context.Context, key IdempotencyKey, expectedActivity [32]byte) (ProgramSubmission, error) { var out ProgramSubmission; err := p.client.call(ctx, PlaneAgent, "program.receipt", false, map[string]any{"idempotency_key": key.String(), "expected_activity_id": expectedActivity, "requested_verification_level": "sequencer-signed"}, &out, CallOptions{}); return out, err }
func (p *Programs) Activity(ctx context.Context, activity [32]byte) (ProgramSubmission, error) { var out ProgramSubmission; err := p.client.call(ctx, PlaneAgent, "program.activity", false, map[string]any{"activity_id": activity, "requested_verification_level": "sequencer-signed"}, &out, CallOptions{}); return out, err }
func PlatformSDKPrograms() string { return "receipt-verified-program-operations-v1" }
