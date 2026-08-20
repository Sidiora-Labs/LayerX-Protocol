# LayerX public testnet

The pending-release testnet runs protocol version `0.1.0`, network ID `402`, at `https://testnet.layerx.network`. The developer gateway is `https://api.testnet.layerx.network`; faucet claims are submitted to `https://faucet.testnet.layerx.network/v1/faucet/claims`. Protocol amounts are integer strings and the public gateway returns typed refusals with retry timing.

State resets at 09:00 UTC on the first Tuesday of every month. The next reset calendar is published as iCalendar at `https://status.layerx.network/testnet-resets.ics`; clients must treat testnet balances and receipts as disposable across that boundary. The deployed image version must equal the pending release version before rollout.

The faucet requires an authenticated developer identity and a destination DID/public key. Durable quotas apply independently to identity, destination address and client network. A successful response means the real testnet core accepted the prefund. `429` responses carry `Retry-After`; unavailable core or durable quota storage returns a typed `503` and never reports funding success.

The status page reports `testnet`, `gateway`, `core` and `paxeer` separately. A gateway outage does not imply a core outage, and Paxeer degradation is not presented as LayerX finality. Operational request logs hash identity, address and client-network quota keys and redact authentication headers.
