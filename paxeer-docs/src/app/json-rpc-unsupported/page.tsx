import { DocsLayout } from '@/components/DocsLayout'
import { FactChips, JumpNav, MethodTable, PageLead, PageNav, Section, SnippetBlock, Subhead, m3 } from '@/components/api/ApiPage'
import Link from 'next/link'

export default function JsonRpcUnsupported() {
  return (
    <DocsLayout pageTitle="Unsupported Methods">
      <PageLead overline="-32000 · registered failures · not -32601" source="paxeer-network/docs/evm_jsonrpc_unsupported.md, paxeer-network/rpc/AGENTS.md">
        <p>
          These Ethereum JSON-RPC names are registered on the Paxeer EVM endpoint and return a JSON-RPC error. Clients get <code>-32000</code> instead of <code>-32601</code> method-not-found.
        </p>
      </PageLead>

      <FactChips
        items={[
          { label: 'Error code', value: '-32000' },
          { label: 'Not-found code (not used)', value: '-32601' },
          { label: 'Local EVM HTTP', value: '127.0.0.1:8545' },
          { label: 'Fixtures', value: 'testdata/<method>/not-supported.iox' },
        ]}
      />

      <JumpNav
        items={[
          { id: 'methods', label: 'Methods' },
          { id: 'error', label: 'Error body' },
          { id: 'notes', label: 'Notes' },
          { id: 'broader', label: 'Broader rules' },
        ]}
      />

      <Section id="methods" title="Registered unsupported methods">
        <MethodTable
          columns={['Method', 'error.message', 'Reason']}
          rows={[
            ['eth_blobBaseFee', 'blobs not supported on this chain', 'No EIP-4844 blobs'],
            ['eth_syncing', 'eth_syncing is not supported on Pax EVM RPC', 'BFT sync is not Ethereum sync'],
            ['eth_newPendingTransactionFilter', 'eth_newPendingTransactionFilter is not supported on Pax EVM RPC', 'No Ethereum-style pending filter'],
            ['debug_getRawBlock', 'debug_getRawBlock is not supported on Pax EVM RPC', 'No raw RLP block'],
            ['debug_getRawHeader', 'debug_getRawHeader is not supported on Pax EVM RPC', 'No raw RLP header'],
            ['debug_getRawReceipts', 'debug_getRawReceipts is not supported on Pax EVM RPC', 'No raw RLP receipts'],
            ['debug_getRawTransaction', 'debug_getRawTransaction is not supported on Pax EVM RPC', 'No raw RLP transaction'],
          ]}
        />
      </Section>

      <Section id="error" title="Error body">
        <SnippetBlock
          method="eth_blobBaseFee"
          args="[]"
          source="paxeer-network/docs/evm_jsonrpc_unsupported.md"
          purpose="Stable -32000 failure for blob fee. Same code for every method in the table."
          code={`{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32000,
    "message": "blobs not supported on this chain"
  }
}`}
        />
      </Section>

      <Section id="notes" title="Method notes">
        <Subhead>eth_syncing</Subhead>
        <p className={m3.body}>
          BFT consensus is not Ethereum sync. Do not poll this method for node catch-up.
        </p>
        <Subhead>eth_newPendingTransactionFilter</Subhead>
        <p className={m3.body}>
          Instant finality. A tx is in a finalized block or it is not yet submitted. There is no pending-filter surface.
        </p>
        <Subhead>debug_getRaw*</Subhead>
        <p className={m3.body}>
          Raw RLP payloads are not served. Use structured <code>eth_*</code> JSON.
        </p>
        <Subhead>eth_blobBaseFee</Subhead>
        <p className={m3.body}>
          EIP-4844 blob transactions are not on this chain.
        </p>
      </Section>

      <Section id="broader" title="Broader compatibility">
        <MethodTable
          columns={['Rule', 'Behavior']}
          rows={[
            ['pending tag', 'Accepted, treated as latest / safe / final'],
            ['Uncle methods', 'Unsupported'],
            ['Trie methods', 'Unsupported (state is not an Ethereum trie)'],
            ['eth_mining / eth_hashrate', 'Unsupported (BFT, not PoW)'],
          ]}
        />
        <p className={m3.body}>
          Integration fixtures: <code>paxeer-network/integration_test/evm_module/rpc_io_test/testdata/&lt;method&gt;/not-supported.iox</code>. Full namespace notes: <Link href="/json-rpc">JSON-RPC</Link>.
        </p>
      </Section>

      <PageNav
        prev={{ href: '/json-rpc', title: 'JSON-RPC' }}
        next={{ href: '/rest-grpc', title: 'REST & gRPC' }}
      />
    </DocsLayout>
  )
}
