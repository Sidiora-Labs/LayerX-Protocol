import { DocsLayout } from '@/components/DocsLayout'
import { Callout, FactChips, JumpNav, MethodTable, PageLead, PageNav, Section, SnippetBlock, Subhead, m3 } from '@/components/api/ApiPage'
import Link from 'next/link'

export default function JsonRpc() {
  return (
    <DocsLayout pageTitle="JSON-RPC">
      <PageLead overline="eth_ · pax_ · pax2_ · debug_ · EVM HTTP :8545" source="paxeer-network/rpc/README.md, paxeer-network/rpc/AGENTS.md, paxeer-network/rpc/pax_legacy.go">
        <p>
          EVM JSON-RPC on the node HTTP port. Docker localnet node0 publishes it at <code>127.0.0.1:8545</code> (<code>paxeer-network/docker/docker-compose.yml</code>). There is no public LayerX RPC on this surface.
        </p>
        <p>
          <code>eth_</code> sees EVM transactions only. <code>pax_</code> adds Cosmos transactions with synthetic receipts. <code>pax2_</code> is the same block shape as <code>pax_</code> plus bank transfers, HTTP only. <code>debug_trace*</code> replays historical execution.
        </p>
      </PageLead>

      <FactChips
        items={[
          { label: 'Local EVM HTTP', value: '127.0.0.1:8545' },
          { label: 'Chain ID', value: '125' },
          { label: 'Gas token', value: 'PAX' },
          { label: 'Legacy gate', value: '[evm].enabled_legacy_pax_apis' },
        ]}
      />

      <JumpNav
        items={[
          { id: 'namespaces', label: 'Namespaces' },
          { id: 'eth', label: 'eth_' },
          { id: 'pax', label: 'pax_' },
          { id: 'pax2', label: 'pax2_' },
          { id: 'indices', label: 'Tx indices' },
          { id: 'debug', label: 'debug_' },
          { id: 'distinctions', label: 'Distinctions' },
          { id: 'websocket', label: 'WebSocket' },
        ]}
      />

      <Section id="namespaces" title="Namespaces">
        <MethodTable
          columns={['Prefix', 'Visibility', 'Use']}
          rows={[
            ['eth_', 'EVM transactions only', 'Ethereum tooling: ethers, viem, web3.js, Hardhat, Foundry'],
            ['pax_', 'EVM + Cosmos with synthetic receipts', 'Indexers that need Cosmos events as EVM logs'],
            ['pax2_', 'EVM + Cosmos + bank transfers in blocks', 'Seven HTTP block methods; no tx or filter API'],
            ['debug_', 'EVM trace replay', 'Historical gas, errors, and state as executed'],
          ]}
        />
      </Section>

      <Section id="eth" title="eth_">
        <p className={m3.body}>
          Pure EVM view. Cosmos-native transactions are omitted. Method names follow the Ethereum JSON-RPC spec except where <Link href="/json-rpc-unsupported">unsupported methods</Link> return <code>-32000</code>.
        </p>
        <MethodTable
          columns={['Method', 'Args', 'Purpose']}
          rows={[
            ['eth_blockNumber', '[]', 'Current EVM head as a hex quantity'],
            ['eth_getBlockByNumber', '[block, fullTx]', 'Block by number or tag'],
            ['eth_getBlockByHash', '[hash, fullTx]', 'Block by hash'],
            ['eth_getTransactionByHash', '[hash]', 'EVM transaction'],
            ['eth_getTransactionReceipt', '[hash]', 'Receipt; also accepts a synthetic hash'],
            ['eth_getLogs', '[filter]', 'EVM logs in a range'],
            ['eth_call', '[tx, block]', 'Read-only call'],
            ['eth_estimateGas', '[tx]', 'Gas estimate'],
            ['eth_sendRawTransaction', '[raw]', 'Submit a signed EVM tx'],
            ['eth_getBalance', '[addr, block]', 'Account balance'],
            ['eth_getCode', '[addr, block]', 'Contract bytecode'],
          ]}
        />
        <SnippetBlock
          method="eth_blockNumber"
          args="[]"
          source="paxeer-network/rpc/"
          purpose="Read the EVM head on the local Docker node0 port."
          code={`curl -s -X POST http://127.0.0.1:8545 \\
  -H 'content-type: application/json' \\
  -d '{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}'`}
        />
      </Section>

      <Section id="pax" title="pax_">
        <p className={m3.body}>
          Cosmos events (CW20, CW721) appear as synthetic EVM logs. For a synthetic transaction, call <code>eth_getTransactionReceipt</code> with that hash. There is no <code>pax_getTransactionByReceipt</code>.
        </p>
        <Subhead>Synthetic logs and blocks</Subhead>
        <MethodTable
          columns={['Method', 'Args', 'Purpose']}
          rows={[
            ['pax_getLogs', '[filter]', 'eth_getLogs plus synthetic logs'],
            ['pax_getFilterLogs', '[id]', 'eth_getFilterLogs plus synthetic logs'],
            ['pax_getBlockByNumber', '[block, fullTx]', 'Block including synthetic txs'],
            ['pax_getBlockByHash', '[hash, fullTx]', 'Block including synthetic txs'],
            ['pax_getBlockReceipts', '[block]', 'Receipts including synthetic txs'],
            ['pax_getPaxAddress', '[evmAddr]', 'Cosmos bech32 from EVM address'],
            ['pax_getEVMAddress', '[paxAddr]', 'EVM address from Cosmos bech32'],
            ['pax_getCosmosTx', '[hash]', 'Cosmos tx by hash'],
          ]}
        />
        <Subhead>ExcludeTraceFail</Subhead>
        <p className={m3.body}>
          Some mempool txs fail pre-state checks (nonce, funds, panic) and land in a block without executing. These methods drop them:
        </p>
        <MethodTable
          columns={['Method', 'Args', 'Purpose']}
          rows={[
            ['pax_traceBlockByNumberExcludeTraceFail', '[block]', 'Trace block, skip pre-state failures'],
            ['pax_traceBlockByHashExcludeTraceFail', '[hash]', 'Trace block by hash, skip failures'],
            ['pax_getTransactionReceiptExcludeTraceFail', '[hash]', 'Receipt if the tx actually executed'],
            ['pax_getBlockByNumberExcludeTraceFail', '[block, fullTx]', 'Block without failed pre-state txs'],
            ['pax_getBlockByHashExcludeTraceFail', '[hash, fullTx]', 'Block without failed pre-state txs'],
          ]}
        />
        <Callout label="Receipt lookup">
          Synthetic txs use <code>eth_getTransactionReceipt</code> with the synthetic hash. Do not invent a <code>pax_</code> receipt-by-receipt method.
        </Callout>
      </Section>

      <Section id="pax2" title="pax2_">
        <p className={m3.body}>
          Seven HTTP methods. Same block JSON as <code>pax_</code>, with bank transfers in the payload. No transaction API, no filter API, no WebSocket.
        </p>
        <MethodTable
          columns={['Method', 'Args', 'Purpose']}
          rows={[
            ['pax2_getBlockByHash', '[hash, fullTx]', 'Block plus bank transfers'],
            ['pax2_getBlockByNumber', '[block, fullTx]', 'Block plus bank transfers'],
            ['pax2_getBlockReceipts', '[block]', 'Receipts plus bank transfers'],
            ['pax2_getBlockTransactionCountByHash', '[hash]', 'Tx count including bank transfers'],
            ['pax2_getBlockTransactionCountByNumber', '[block]', 'Tx count including bank transfers'],
            ['pax2_getBlockByHashExcludeTraceFail', '[hash, fullTx]', 'Block minus pre-state failures'],
            ['pax2_getBlockByNumberExcludeTraceFail', '[block, fullTx]', 'Block minus pre-state failures'],
          ]}
        />
        <Subhead>Legacy gate</Subhead>
        <p className={m3.body}>
          Every gated <code>pax_*</code> and <code>pax2_*</code> name is allowlisted by <code>[evm].enabled_legacy_pax_apis</code> in <code>app.toml</code>. Both prefixes share one list. The surfaces are deprecated and scheduled for removal. <code>paxd init</code> pre-fills <code>pax_getPaxAddress</code>, <code>pax_getEVMAddress</code>, and <code>pax_getCosmosTx</code>. Docker localnet enables every gated method except <code>pax_sign</code> (<code>paxeer-network/docker/localnode/config/app.toml</code>).
        </p>
        <p className={m3.body}>
          A disabled method returns HTTP 200 with JSON-RPC <code>-32601</code>, <code>data: "legacy_pax_deprecated"</code>. Allowed single-object bodies pass through unchanged. Allowed responses may set header <code>Pax-Legacy-RPC-Deprecation</code>.
        </p>
        <SnippetBlock
          method="pax_getLogs"
          args="[filter]"
          source="paxeer-network/rpc/pax_legacy.go"
          purpose="Legacy pax_* call. Fails with -32601 unless the method is on the allowlist."
          code={`# Disabled methods (not on enabled_legacy_pax_apis):
# { "jsonrpc":"2.0", "id":1, "error": { "code":-32601, "data":"legacy_pax_deprecated" } }
#
# Coverage: paxeer-network/rpc/pax_legacy_test.go
# and integration_test/evm_module/rpc_io_test/testdata/pax_legacy_deprecation/*.iox`}
        />
      </Section>

      <Section id="indices" title="Transaction indices">
        <p className={m3.body}>
          Indices are namespace-local. Hashes are stable across namespaces; indices are not.
        </p>
        <div className="grid grid-cols-2 gap-3 my-6">
          <div className="rounded-lg bg-surface-container shadow-1 px-4 py-4">
            <div className={m3.overline}>eth_getBlockReceipts</div>
            <p className={`${m3.body} mt-3`}>EVM Transaction 1 → index 0</p>
            <p className={m3.body}>EVM Transaction 2 → index 1</p>
            <p className={`${m3.label} mt-3`}>Cosmos tx omitted</p>
          </div>
          <div className="rounded-lg bg-surface-container shadow-1 px-4 py-4">
            <div className={m3.overline}>pax_getBlockReceipts</div>
            <p className={`${m3.body} mt-3`}>EVM Transaction 1 → index 0</p>
            <p className={m3.body}>Cosmos Transaction 1 → index 1</p>
            <p className={m3.body}>EVM Transaction 2 → index 2</p>
          </div>
        </div>
        <ul>
          <li>EVM-originating synthetic events appear in both <code>eth_getLogs</code> and <code>eth_getTransactionReceipt</code>.</li>
          <li>Cosmos-originating synthetic events are not in <code>eth_</code> methods. Use <code>pax_getLogs</code> and <code>pax_getBlockReceipts</code>.</li>
          <li><code>logIndex</code> is strictly increasing inside one namespace.</li>
        </ul>
      </Section>

      <Section id="debug" title="debug_">
        <p className={m3.body}>
          <code>debug_trace*</code> replays the historical execution. If the tx errored, the trace errors. Gas used matches the executed amount.
        </p>
        <MethodTable
          columns={['Method', 'Args', 'Purpose']}
          rows={[
            ['debug_traceBlockByNumber', '[block, opts]', 'Replay every tx in a block'],
            ['debug_traceBlockByHash', '[hash, opts]', 'Replay by block hash'],
            ['debug_traceTransaction', '[hash, opts]', 'Replay one tx'],
            ['debug_traceCall', '[tx, block, opts]', 'Trace a call against a block'],
          ]}
        />
      </Section>

      <Section id="distinctions" title="Distinctions from Ethereum">
        <MethodTable
          columns={['Rule', 'Behavior']}
          rows={[
            ['No pending state', 'pending is accepted and treated as latest / safe / final'],
            ['No uncle blocks', 'BFT finality. Uncle methods are unsupported'],
            ['No trie', 'State is not an Ethereum trie. Trie methods are unsupported'],
            ['No PoW', 'eth_mining and eth_hashrate are unsupported'],
            ['No blobs', 'eth_blobBaseFee returns -32000: blobs not supported on this chain'],
            ['Historical stability', 'Responses at a height do not change after upgrades'],
          ]}
        />
        <p className={m3.body}>
          Registered failures live on <Link href="/json-rpc-unsupported">Unsupported Methods</Link>.
        </p>
      </Section>

      <Section id="websocket" title="WebSocket">
        <p className={m3.body}>
          <code>eth_subscribe</code> and <code>eth_unsubscribe</code> accept <code>newHeads</code>, <code>logs</code>, and <code>newPendingTransactions</code>. <code>pax2_*</code> is HTTP only.
        </p>
      </Section>

      <PageNav
        prev={{ href: '/wasmbinding', title: 'WASM Bindings' }}
        next={{ href: '/json-rpc-unsupported', title: 'Unsupported Methods' }}
      />
    </DocsLayout>
  )
}
