# LayerX portable verification formats

`layerx-receipt-proof-v1` is a UTF-8 JSON object with exactly these
camel-cased members, in any JSON member order:

| Member | Value |
|---|---|
| `format` | Literal `layerx-receipt-proof-v1` |
| `verificationLevel` | Literal `sequencer-signed` |
| `canonicalReceipt` | The complete canonical LayerX receipt, unpadded base64url |
| `receiptDigest` | The 32-byte receipt-signature digest, unpadded base64url |
| `batchId` | The claimed 32-byte authorised batch id, unpadded base64url |
| `asset` | The claimed 32-byte batch asset, unpadded base64url |
| `previousStateRoot` | The claimed 32-byte predecessor root, unpadded base64url |
| `resultingStateRoot` | The claimed 32-byte successor root, unpadded base64url |
| `sequencerPublicKey` | The claimed 32-byte Ed25519 key, unpadded base64url |

An independent verifier must reject unknown members, padded or non-canonical
base64url, wrong fixed-field lengths, and canonical receipts above 1,048,576
bytes. The object is not its own trust root: before verification, the verifier
must obtain the authorised batch id, asset, root chain and sequencer key from
its own trusted snapshot, certificate or vector manifest. It then:

1. Decodes the canonical receipt and re-encodes it byte-for-byte under the
   LayerX receipt codec. A difference is a refusal.
2. Requires a full protocol receipt, protocol version 1, a non-zero operation
   and activity id, and a sequencer signature.
3. Requires every claimed batch field in the object and receipt to equal the
   independently trusted batch authorization.
4. For a successful result, checks the debit decreases and credit increases by
   exactly the receipt amount using checked unsigned integer arithmetic.
5. Encodes the same receipt without its sequencer signature and computes
   `SHA-256("LXP/v1/receipt\0" || unsigned_canonical_receipt)`.
6. Requires that digest to equal `receiptDigest`, then strictly verifies the
   Ed25519 signature over the 32-byte digest with `sequencerPublicKey`.

A verified rejected outcome remains rejected. This format proves a
sequencer-signed receipt; it does not claim batch inclusion, checkpoint
finality or external settlement.

External mandates and receipts retain their upstream wire formats. LayerX does
not wrap or reinterpret their claims. `ExternalPresentation` names the exact
adapter, protocol version, media type and claim class; the matching adapter
performs its protocol-specific signature, delegation, time, nonce, audience
and constraint checks. `verify_external_evidence` returns the adapter's typed
verified value bound to the exact presentation bytes and pinned upstream
specification and conformance-suite digests. It retains only that
domain-separated digest, never the possibly secret-bearing raw mandate.

AP2 Checkout and Payment Mandates must be verified as a bound pair. The AP2
adapter therefore accepts
`application/vnd.layerx.ap2-mandate-pair+json`, an object containing exactly
`checkoutMandate` and `paymentMandate`. Their string contents are the unchanged
upstream SD-JWT presentations; the JSON object is only a LayerX transport
envelope. `Ap2ExternalMandateVerifier` passes both exact strings into AP2's
signature, selective-disclosure, delegation and constraint verifier and
returns its private `VerifiedMandates` type through the generic binding.
