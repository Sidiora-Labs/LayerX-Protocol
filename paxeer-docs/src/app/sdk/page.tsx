import { DocsLayout } from '@/components/DocsLayout'
import { Callout, FactChips, JumpNav, MethodTable, PageLead, PageNav, Section, SnippetBlock, m3 } from '@/components/api/ApiPage'
import Link from 'next/link'

export default function SDK() {
  return (
    <DocsLayout pageTitle="SDK">
      <PageLead overline="github.com/sidiora-labs/paxeer-network/sdk · in-tree fork" source="paxeer-network/sdk/">
        <p>
          In-tree Cosmos SDK fork used by <code>paxd</code>. Not a published Go module. Import path is <code>github.com/sidiora-labs/paxeer-network/sdk</code>.
        </p>
        <p>
          It is not drop-in compatible with upstream Cosmos SDK releases. Test CosmJS, Keplr, and other upstream tools against this fork before assuming they work.
        </p>
      </PageLead>

      <FactChips
        items={[
          { label: 'Module path', value: 'github.com/sidiora-labs/paxeer-network/sdk' },
          { label: 'Binary', value: 'paxd' },
          { label: 'gRPC default', value: ':9090' },
          { label: 'REST default', value: ':1317' },
        ]}
      />

      <JumpNav
        items={[
          { id: 'tree', label: 'Tree' },
          { id: 'modules', label: 'Modules' },
          { id: 'baseapp', label: 'BaseApp' },
          { id: 'store', label: 'Store' },
          { id: 'build', label: 'Build' },
          { id: 'fork', label: 'Fork deltas' },
        ]}
      />

      <Section id="tree" title="Directory">
        <MethodTable
          columns={['Path', 'Purpose']}
          rows={[
            ['baseapp/', 'ABCI, routing, ante handlers'],
            ['client/', 'CLI and tx builders'],
            ['server/', 'Node server, gRPC, REST gateway'],
            ['types/', 'Messages, coins, errors, context'],
            ['x/', 'Upstream Cosmos modules'],
            ['proto/', 'SDK protobufs'],
            ['store/', 'Store abstraction'],
            ['simapp/', 'Reference app for tests'],
          ]}
        />
      </Section>

      <Section id="modules" title="Modules">
        <p className={m3.body}>Upstream modules in the fork:</p>
        <MethodTable
          columns={['Module', 'Purpose']}
          rows={[
            ['x/auth', 'Accounts and signatures'],
            ['x/bank', 'Transfers and balances'],
            ['x/staking', 'Validator set and delegation'],
            ['x/distribution', 'Rewards and fees'],
            ['x/gov', 'Proposals'],
            ['x/slashing', 'Validator penalties'],
            ['x/crisis', 'Invariants and halt'],
            ['x/evidence', 'Double-sign evidence'],
            ['x/params', 'Module parameters'],
            ['x/upgrade', 'Coordinated upgrades'],
          ]}
        />
        <p className={m3.body}>
          Paxeer modules live under <code>paxeer-network/modules/</code>: <code>evm</code>, <code>epoch</code>, <code>oracle</code>, <code>tokenfactory</code>, <code>mint</code>. Query RPCs: <Link href="/rest-grpc">REST & gRPC</Link>.
        </p>
      </Section>

      <Section id="baseapp" title="BaseApp, client, server">
        <p className={m3.body}>
          <code>baseapp/</code> owns ABCI, message routing, ante (gas, signatures, nonces), commits, and queries. Paxeer extends it for EVM routing and OCC. Client/server expose <code>paxd</code> CLI, module gRPC, REST gateway, and Tendermint RPC.
        </p>
      </Section>

      <Section id="store" title="Store">
        <p className={m3.body}>
          <code>store/</code> is implemented by Paxeer engines: MEMIAVL (legacy in-memory IAVL) and Giga (flat KV). Details: <Link href="/storage">Storage</Link>.
        </p>
      </Section>

      <Section id="build" title="Build against the fork">
        <SnippetBlock
          method="github.com/sidiora-labs/paxeer-network/sdk"
          args="no go.mod replace"
          source="paxeer-network/sdk/"
          purpose="Import the in-tree SDK. The monorepo already contains the module."
          code={`import (
    sdk "github.com/sidiora-labs/paxeer-network/sdk/types"
    "github.com/sidiora-labs/paxeer-network/sdk/store"
)`}
        />
        <SnippetBlock
          method="make build"
          args="paxd → ./build/paxd"
          source="paxeer-network/Makefile, paxeer-network/api/README.md"
          purpose="Build paxd, run tests, and regenerate protos from the network tree."
          code={`make build
make test
make proto-gen

cd paxeer-network
ignite generate proto-go`}
        />
        <p className={m3.body}>
          Ignite CLI v0.23.0. SDK docs under <code>sdk/docs/</code> are upstream text and may omit Paxeer deltas. Upstream cosmos.network docs describe the public SDK, not this fork.
        </p>
      </Section>

      <Section id="fork" title="Fork deltas">
        <ul>
          <li>Storage backends: MEMIAVL, Giga</li>
          <li>EVM module and pointer contracts</li>
          <li>OCC parallel execution</li>
          <li>Receipt indexing and synthetic transactions</li>
          <li>Paxeer ante handlers and gas metering</li>
          <li>Chain-specific upgrade logic</li>
        </ul>
        <Callout label="Changes in sdk/">
          Edit, <code>make build</code>, <code>make test</code>, Docker cluster (<Link href="/docker">Docker</Link>), then <code>make test-integration</code>. A fork change hits every <code>paxd</code> path.
        </Callout>
      </Section>

      <PageNav
        prev={{ href: '/docker', title: 'Docker' }}
        next={{ href: '/interchain', title: 'Interchain' }}
      />
    </DocsLayout>
  )
}
