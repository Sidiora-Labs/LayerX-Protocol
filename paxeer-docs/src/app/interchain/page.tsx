import { DocsLayout } from '@/components/DocsLayout'
import { Callout, FactChips, JumpNav, MethodTable, PageLead, PageNav, Section, SnippetBlock, Subhead, m3 } from '@/components/api/ApiPage'
import Link from 'next/link'

export default function Interchain() {
  return (
    <DocsLayout pageTitle="Interchain">
      <PageLead overline="paxeer-network/interchain/ · ibc-go fork · hyperpax_125-1" source="paxeer-network/interchain/">
        <p>
          In-tree ibc-go fork for <code>paxd</code>. Not a published module. IBC is Cosmos-level: client, connection, channel, then packets. EVM contracts do not send or receive IBC packets directly.
        </p>
        <p>
          Relayers need chain ID <code>hyperpax_125-1</code> (EVM 125) and a Paxeer RPC you operate. There is no public LayerX RPC on this page.
        </p>
      </PageLead>

      <FactChips
        items={[
          { label: 'Tree', value: 'paxeer-network/interchain/' },
          { label: 'Chain ID', value: 'hyperpax_125-1' },
          { label: 'EVM chain ID', value: '125' },
          { label: 'Transfer app', value: 'ICS-20' },
        ]}
      />

      <JumpNav
        items={[
          { id: 'core', label: 'Core' },
          { id: 'apps', label: 'Apps' },
          { id: 'clients', label: 'Light clients' },
          { id: 'tree', label: 'Tree' },
          { id: 'cli', label: 'CLI' },
          { id: 'evm', label: 'IBC and EVM' },
        ]}
      />

      <Section id="core" title="Core stack">
        <MethodTable
          columns={['Component', 'ICS', 'Purpose']}
          rows={[
            ['Client', 'ICS-02', 'Light-client verification of a remote chain'],
            ['Connection', 'ICS-03', 'Authenticated connection'],
            ['Channel', 'ICS-04', 'Ordered or unordered packet delivery'],
            ['Port', 'ICS-05', 'Module registration and routing'],
            ['Commitment', 'ICS-23', 'Proofs'],
            ['Host', 'ICS-24', 'Host requirements'],
          ]}
        />
      </Section>

      <Section id="apps" title="Applications">
        <Subhead>ICS-20 transfer</Subhead>
        <p className={m3.body}>
          <code>modules/apps/transfer</code> moves tokens. Paxeer-origin assets leave as ICS-20 packets. Remote assets arrive as IBC-prefixed denoms.
        </p>
        <Subhead>ICS-27 interchain accounts</Subhead>
        <p className={m3.body}>
          One chain controls an account on another. Used for remote staking and governance, not for LayerX settlement.
        </p>
      </Section>

      <Section id="clients" title="Light clients">
        <MethodTable
          columns={['Client', 'ICS', 'Purpose']}
          rows={[
            ['Tendermint', 'ICS-07', 'Tendermint / CometBFT counterparties'],
            ['Solo machine', 'ICS-06', 'Single-signer counterparties'],
          ]}
        />
        <Callout label="09-localhost">
          <code>paxeer-network/interchain/README.md</code> states the localhost client is currently non-functional in this fork.
        </Callout>
      </Section>

      <Section id="tree" title="Tree">
        <MethodTable
          columns={['Path', 'Purpose']}
          rows={[
            ['modules/core/', 'Client, connection, channel'],
            ['modules/apps/', 'transfer, interchain accounts'],
            ['modules/light-clients/', 'ICS-07, ICS-06, 09-localhost'],
            ['proto/', 'IBC protobufs'],
          ]}
        />
        <p className={m3.body}>
          Upstream version is the ibc-go pin in <code>paxeer-network/interchain/go.mod</code>. Do not assume latest upstream ibc-go.
        </p>
      </Section>

      <Section id="cli" title="CLI">
        <p className={m3.body}>
          Open a client, a connection, a channel, then send packets. Relayers (Hermes, Go Relayer) ferry proofs. Query live channel IDs; do not assume <code>channel-0</code>.
        </p>
        <SnippetBlock
          method="paxd tx ibc-transfer transfer"
          args="port channel recipient amount --chain-id hyperpax_125-1"
          source="paxeer-network/interchain/modules/apps/transfer/"
          purpose="ICS-20 send. Replace channel, recipient, and amount. Native microdenom in-tree is uhpx; PAX is gas."
          code={`paxd tx ibc-transfer transfer \\
  transfer \\
  <channel-id> \\
  <recipient> \\
  <amount>uhpx \\
  --from <key> \\
  --chain-id hyperpax_125-1`}
        />
        <SnippetBlock
          method="paxd query ibc"
          args="client | connection | channel"
          source="paxeer-network/interchain/"
          purpose="Read IBC clients, connections, and channels from a running node."
          code={`paxd query ibc client states
paxd query ibc connection connections
paxd query ibc channel channels`}
        />
        <p className={m3.body}>
          Relayer security depends on light-client correctness, relayer liveness (timeouts), and each chain's own IBC upgrades. Coordinate upgrades with counterparties.
        </p>
      </Section>

      <Section id="evm" title="IBC and EVM">
        <p className={m3.body}>
          Solidity cannot emit IBC packets. Bridge paths that exist in this repo: pointer contracts for Cosmos tokens, precompiles for module calls, or a Cosmos tx that talks to IBC. See <Link href="/contracts">Contracts</Link>.
        </p>
      </Section>

      <PageNav
        prev={{ href: '/sdk', title: 'SDK' }}
        next={{ href: '/admin-hpx', title: 'Admin & HPX' }}
      />
    </DocsLayout>
  )
}
