import { DocsLayout } from '@/components/DocsLayout'
import { Callout, FactChips, JumpNav, MethodTable, PageLead, PageNav, Section, SnippetBlock, Subhead, m3 } from '@/components/api/ApiPage'
import Link from 'next/link'

export default function RestGrpc() {
  return (
    <DocsLayout pageTitle="REST & gRPC">
      <PageLead overline="gRPC :9090 · REST :1317 · proto Query services" source="paxeer-network/api/">
        <p>
          Cosmos query services generated from <code>paxeer-network/api/</code>. Default gRPC bind is <code>0.0.0.0:9090</code>. Default REST gateway is <code>tcp://0.0.0.0:1317</code> (<code>paxeer-network/sdk/server/config/config.go</code>). Docker localnet node0 publishes gRPC at <code>9090-9091</code>, not 1317.
        </p>
        <p>
          JSON-RPC on <code>:8545</code> is a different process. LayerX activities are not queried here. LayerX fee is 5,000 µUSDX per activity, not zero. PAX is gas on this chain.
        </p>
      </PageLead>

      <FactChips
        items={[
          { label: 'gRPC default', value: '0.0.0.0:9090' },
          { label: 'REST default', value: 'tcp://0.0.0.0:1317' },
          { label: 'Docker node0 gRPC', value: '9090-9091' },
          { label: 'Admin gRPC', value: '127.0.0.1:9095' },
        ]}
      />

      <JumpNav
        items={[
          { id: 'codegen', label: 'Codegen' },
          { id: 'evm', label: 'evm.Query' },
          { id: 'epoch', label: 'epoch.Query' },
          { id: 'oracle', label: 'oracle.Query' },
          { id: 'tokenfactory', label: 'tokenfactory.Query' },
          { id: 'mint', label: 'mint.Query' },
          { id: 'admin', label: 'AdminService' },
          { id: 'swagger', label: 'Swagger' },
        ]}
      />

      <Section id="codegen" title="Code generation">
        <p className={m3.body}>
          Proto files emit Go types under <code>github.com/sidiora-labs/paxeer-network/modules/*/types</code>, gRPC interfaces, REST via <code>google.api.http</code>, and OpenAPI. Regen:
        </p>
        <SnippetBlock
          method="ignite generate proto-go"
          args="Ignite CLI v0.23.0"
          source="paxeer-network/api/README.md"
          purpose="Regenerate Go types, gRPC stubs, and REST annotations from api/ protos."
          code={`ignite generate proto-go`}
        />
      </Section>

      <Section id="evm" title="paxprotocol.paxchain.evm.Query">
        <p className={m3.body}>Address maps, static calls, pointer lookups. Proto: <code>paxeer-network/api/evm/query.proto</code>.</p>
        <MethodTable
          columns={['RPC', 'REST', 'Purpose']}
          rows={[
            ['PaxAddressByEVMAddress', 'GET /pax-protocol/paxchain/evm/pax_address', 'Cosmos address from EVM address'],
            ['EVMAddressByPaxAddress', 'GET /pax-protocol/paxchain/evm/evm_address', 'EVM address from Cosmos address'],
            ['StaticCall', 'GET /pax-protocol/paxchain/evm/static_call', 'Read-only contract call'],
            ['Pointer', 'GET /pax-protocol/paxchain/evm/pointer', 'EVM pointer for a Cosmos contract'],
            ['Pointee', 'GET /pax-protocol/paxchain/evm/pointee', 'Cosmos contract from an EVM pointer'],
            ['PointerVersion', 'GET /pax-protocol/paxchain/evm/pointer_version', 'Current pointer contract version'],
          ]}
        />
        <SnippetBlock
          method="paxprotocol.paxchain.evm.Query/Pointer"
          args="gRPC :9090"
          source="paxeer-network/api/evm/query.proto"
          purpose="List services, then call Pointer on the default gRPC port."
          code={`grpcurl -plaintext localhost:9090 list
grpcurl -plaintext localhost:9090 paxprotocol.paxchain.evm.Query/Pointer`}
        />
      </Section>

      <Section id="epoch" title="paxprotocol.paxchain.epoch.Query">
        <p className={m3.body}>Proto: <code>paxeer-network/api/epoch/query.proto</code>.</p>
        <MethodTable
          columns={['RPC', 'REST', 'Purpose']}
          rows={[
            ['Epoch', 'GET /pax-protocol/paxchain/epoch/epoch', 'Current epoch number and metadata'],
            ['Params', 'GET /pax-protocol/paxchain/epoch/params', 'Epoch module parameters'],
          ]}
        />
      </Section>

      <Section id="oracle" title="oracle Query">
        <p className={m3.body}>
          HTTP paths use <code>/pax-protocol/pax-chain/oracle/...</code> (hyphen). Proto: <code>paxeer-network/api/oracle/query.proto</code>.
        </p>
        <Subhead>Rates</Subhead>
        <MethodTable
          columns={['RPC', 'REST', 'Purpose']}
          rows={[
            ['ExchangeRate', 'GET /pax-protocol/pax-chain/oracle/denoms/{denom}/exchange_rate', 'Rate for one denom'],
            ['ExchangeRates', 'GET /pax-protocol/pax-chain/oracle/denoms/exchange_rates', 'All rates'],
            ['Actives', 'GET /pax-protocol/pax-chain/oracle/denoms/actives', 'Active denoms'],
            ['VoteTargets', 'GET /pax-protocol/pax-chain/oracle/denoms/vote_targets', 'Vote-target denoms'],
            ['PriceSnapshotHistory', 'GET /pax-protocol/pax-chain/oracle/denoms/price_snapshot_history', 'Price snapshots'],
            ['Twaps', 'GET /pax-protocol/pax-chain/oracle/denoms/twaps/{lookback_seconds}', 'TWAP over lookback'],
          ]}
        />
        <Subhead>Validators</Subhead>
        <MethodTable
          columns={['RPC', 'REST', 'Purpose']}
          rows={[
            ['FeederDelegation', 'GET /pax-protocol/pax-chain/oracle/validators/{validator_addr}/feeder', 'Feeder for a validator'],
            ['VotePenaltyCounter', 'GET /pax-protocol/pax-chain/oracle/validators/{validator_addr}/vote_penalty_counter', 'Oracle miss counter'],
            ['SlashWindow', 'GET /pax-protocol/pax-chain/oracle/slash_window', 'Slash window'],
            ['Params', 'GET /pax-protocol/pax-chain/oracle/params', 'Oracle parameters'],
          ]}
        />
      </Section>

      <Section id="tokenfactory" title="paxprotocol.paxchain.tokenfactory.Query">
        <p className={m3.body}>Proto: <code>paxeer-network/api/tokenfactory/query.proto</code>.</p>
        <MethodTable
          columns={['RPC', 'REST', 'Purpose']}
          rows={[
            ['Params', 'GET /pax-protocol/paxchain/tokenfactory/params', 'Tokenfactory parameters'],
            ['DenomAuthorityMetadata', 'GET /pax-protocol/paxchain/tokenfactory/denoms/{denom}/authority_metadata', 'Denom authority'],
            ['DenomMetadata', 'GET /pax-protocol/paxchain/tokenfactory/denoms/metadata', 'Denom metadata'],
            ['DenomsFromCreator', 'GET /pax-protocol/paxchain/tokenfactory/denoms_from_creator/{creator}', 'Denoms created by an address'],
            ['DenomAllowList', 'GET /pax-protocol/paxchain/tokenfactory/denoms/allow_list', 'Allow list'],
          ]}
        />
      </Section>

      <Section id="mint" title="mint.v1beta1.Query">
        <p className={m3.body}>Proto: <code>paxeer-network/api/mint/v1beta1/query.proto</code>. Paths omit the <code>pax-protocol</code> prefix.</p>
        <MethodTable
          columns={['RPC', 'REST', 'Purpose']}
          rows={[
            ['Params', 'GET /paxchain/mint/v1beta1/params', 'Mint parameters'],
            ['Minter', 'GET /paxchain/mint/v1beta1/minter', 'Minter state'],
          ]}
        />
      </Section>

      <Section id="admin" title="paxprotocol.paxchain.admin.v0.AdminService">
        <p className={m3.body}>
          Loopback-only gRPC. Default <code>127.0.0.1:9095</code>. Binding a non-loopback address is rejected at startup. Proto: <code>paxeer-network/api/pax/admin/v0/admin.proto</code>. Implementation: <code>paxeer-network/admin/</code>.
        </p>
        <MethodTable
          columns={['RPC', 'Request', 'Purpose']}
          rows={[
            ['SetLogLevel', 'pattern, level', 'Set level for matching loggers; returns affected'],
            ['GetLogLevel', 'logger', 'Current level for one logger'],
            ['ListLoggers', 'prefix (optional)', 'Registered loggers and levels'],
          ]}
        />
        <SnippetBlock
          method="[admin_server]"
          args="admin_address = 127.0.0.1:9095"
          source="paxeer-network/admin/config.go"
          purpose="Enable the loopback admin server in app.toml."
          code={`[admin_server]
admin_enabled = true
admin_address = "127.0.0.1:9095"`}
        />
        <Callout label="REST vs admin">
          Admin is not on the :1317 gateway. Use gRPC on loopback, or see <Link href="/admin-hpx">Admin & HPX</Link>.
        </Callout>
      </Section>

      <Section id="swagger" title="OpenAPI">
        <p className={m3.body}>
          Regen embeds <code>docs/swagger-ui/swagger.yml</code> into <code>docs/swagger/statik.go</code>. Instructions: <code>paxeer-network/docs/README.md</code>.
        </p>
        <SnippetBlock
          method="./scripts/update-swagger-ui-statik.sh"
          args="swagger = true under [api]"
          source="paxeer-network/docs/README.md"
          purpose="Rebuild the embedded Swagger UI and serve it from the API listener."
          code={`./scripts/update-swagger-ui-statik.sh

[api]
enable = true
swagger = true

# UI path: http://<node-ip>:<api-port>/swagger/`}
        />
        <p className={m3.body}>
          Msg services in <code>evm/tx.proto</code>, <code>epoch/tx.proto</code>, and the other module <code>tx.proto</code> files are transaction types, not query RPCs. Submit them through Cosmos tx APIs.
        </p>
      </Section>

      <PageNav
        prev={{ href: '/json-rpc-unsupported', title: 'Unsupported Methods' }}
        next={{ href: '/contracts', title: 'Contracts' }}
      />
    </DocsLayout>
  )
}
