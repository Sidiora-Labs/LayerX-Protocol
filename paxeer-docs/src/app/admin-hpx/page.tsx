import { DocsLayout } from '@/components/DocsLayout'
import { Callout, FactChips, JumpNav, MethodTable, PageLead, PageNav, Section, SnippetBlock, Subhead, m3 } from '@/components/api/ApiPage'

export default function AdminHpx() {
  return (
    <DocsLayout pageTitle="Admin & HPX">
      <PageLead overline="AdminService :9095 · hpx · node.hyperpaxeer.com" source="paxeer-network/admin/, paxeer-network/hpx/">
        <p>
          Two surfaces. <code>AdminService</code> is loopback gRPC at <code>127.0.0.1:9095</code> for live log levels. HPX installs a native <code>paxd</code> for <code>hyperpax_125-1</code> (EVM 125) from <code>https://node.hyperpaxeer.com</code>.
        </p>
        <p>
          Artifacts live outside git. PAX is gas. LayerX fee is 5,000 µUSDX per activity, not zero. HPX is not a LayerX RPC.
        </p>
      </PageLead>

      <FactChips
        items={[
          { label: 'Admin gRPC', value: '127.0.0.1:9095' },
          { label: 'Proto', value: 'paxprotocol.paxchain.admin.v0' },
          { label: 'HPX origin', value: 'node.hyperpaxeer.com' },
          { label: 'Chain ID', value: 'hyperpax_125-1' },
        ]}
      />

      <JumpNav
        items={[
          { id: 'admin', label: 'AdminService' },
          { id: 'grpcurl', label: 'grpcurl' },
          { id: 'hpx', label: 'HPX install' },
          { id: 'cli', label: 'hpx CLI' },
          { id: 'registry', label: 'Registry' },
          { id: 'publish', label: 'Publish' },
        ]}
      />

      <Section id="admin" title="AdminService">
        <p className={m3.body}>
          Runtime log control without restarting <code>paxd</code>. Address must be loopback (<code>127.0.0.1</code> or <code>::1</code>). Non-loopback binds are rejected. Proto: <code>paxeer-network/api/pax/admin/v0/admin.proto</code>. Code: <code>paxeer-network/admin/server.go</code>, <code>service.go</code>, <code>config.go</code>.
        </p>
        <SnippetBlock
          method="[admin_server]"
          args="admin_address = 127.0.0.1:9095"
          source="paxeer-network/admin/config.go"
          purpose="Enable the loopback-only admin gRPC server."
          code={`[admin_server]
admin_enabled = true
admin_address = "127.0.0.1:9095"`}
        />
        <MethodTable
          columns={['RPC', 'Request', 'Response', 'Purpose']}
          rows={[
            ['SetLogLevel', 'pattern, level', 'affected count', 'Set level for matching loggers'],
            ['GetLogLevel', 'logger', 'level', 'Read one logger'],
            ['ListLoggers', 'prefix (optional)', 'loggers[]', 'List registered loggers'],
          ]}
        />
        <Subhead>Patterns</Subhead>
        <p className={m3.body}>
          Exact <code>evm</code>, glob <code>evm*</code>, or <code>*</code> for every logger. Levels: <code>debug</code>, <code>info</code>, <code>warn</code>, <code>error</code>.
        </p>
        <p className={m3.body}>
          No auth on loopback. For a remote host, tunnel: <code>ssh -L 9095:127.0.0.1:9095 user@node-ip</code>.
        </p>
      </Section>

      <Section id="grpcurl" title="grpcurl">
        <SnippetBlock
          method="paxprotocol.paxchain.admin.v0.AdminService/ListLoggers"
          args="localhost:9095"
          source="paxeer-network/api/pax/admin/v0/admin.proto"
          purpose="List loggers, set evm to debug, then read the evm level back."
          code={`grpcurl -plaintext localhost:9095 \\
  paxprotocol.paxchain.admin.v0.AdminService/ListLoggers

grpcurl -plaintext -d '{"pattern":"evm","level":"debug"}' \\
  localhost:9095 \\
  paxprotocol.paxchain.admin.v0.AdminService/SetLogLevel

grpcurl -plaintext -d '{"logger":"evm"}' \\
  localhost:9095 \\
  paxprotocol.paxchain.admin.v0.AdminService/GetLogLevel`}
        />
      </Section>

      <Section id="hpx" title="HPX">
        <p className={m3.body}>
          Public installer, node manager, artifact publisher, and peer registry. Nodes run native <code>paxd</code> under systemd. Generated binaries, libs, live config, registry state, and TLS stay outside git.
        </p>
        <SnippetBlock
          method="get-hpx.sh"
          args="https://node.hyperpaxeer.com"
          source="paxeer-network/hpx/README.md"
          purpose="Install the hpx CLI after checksum verification, then set up a fullnode."
          code={`curl -sSL https://node.hyperpaxeer.com/get-hpx.sh | sudo bash
export HPX_TYPE=fullnode
hpx setup`}
        />
        <p className={m3.body}>
          Setup pulls <code>paxd</code>, libwasmvm runtimes, genesis, and config; verifies <code>checksums.txt</code>; installs under <code>/root/.paxeer/</code>; enables systemd; starts <code>paxd</code>.
        </p>
      </Section>

      <Section id="cli" title="hpx CLI">
        <MethodTable
          columns={['Command', 'Purpose']}
          rows={[
            ['hpx status', 'Sync status'],
            ['hpx info', 'Node config'],
            ['hpx logs', 'paxd logs'],
            ['hpx update', 'Update paxd and libs'],
            ['hpx peers show', 'Known peers'],
            ['hpx peers refresh', 'Pull the registry peer list'],
            ['hpx register', 'Announce this node'],
            ['hpx statesync', 'Write state-sync trust params into config.toml'],
            ['hpx remove', 'Stop paxd, drop systemd, delete /root/.paxeer/'],
          ]}
        />
        <MethodTable
          columns={['HPX_TYPE', 'Role', 'Config']}
          rows={[
            ['fullnode', 'Non-validating node', '/config/fullnode/'],
            ['validator', 'Validating node', '/config/validator/'],
          ]}
        />
      </Section>

      <Section id="registry" title="Registry">
        <p className={m3.body}>
          Origin <code>https://node.hyperpaxeer.com</code>. Paths from <code>paxeer-network/hpx/README.md</code> and <code>paxeer-network/hpx/registry/main.go</code>.
        </p>
        <MethodTable
          columns={['Method', 'Path', 'Purpose']}
          rows={[
            ['GET', '/healthz', 'Liveness, chain, source revision'],
            ['GET', '/checksums.txt', 'SHA-256 manifest'],
            ['GET', '/chain-info.json', 'Chain metadata'],
            ['GET', '/paxd', 'Native paxd'],
            ['GET', '/lib/*.so', 'Declared libwasmvm runtimes'],
            ['GET', '/genesis.json', 'Genesis'],
            ['GET', '/config/<type>/<file>', 'config.toml / app.toml'],
            ['GET', '/api/myip', 'Caller public address'],
            ['POST', '/api/register', 'Announce peer address'],
            ['GET', '/api/peers', 'JSON peers'],
            ['GET', '/api/peers.txt', 'Tendermint peer text'],
            ['GET', '/api/nodes', 'Node metadata'],
            ['GET', '/api/statesync', 'State-sync trust parameters'],
          ]}
        />
        <p className={m3.body}>
          Served libs: <code>libwasmvm.x86_64.so</code>, <code>libwasmvm.aarch64.so</code>, <code>libwasmvm152.*.so</code>, <code>libwasmvm155.*.so</code>. Directory indexes and undeclared paths are denied.
        </p>
        <Callout label="Register token">
          Public by default. Set <code>HPX_REGISTER_TOKEN</code> before <code>hosting/deploy.sh</code> to require <code>X-HPX-Token</code> on <code>POST /api/register</code>.
        </Callout>
      </Section>

      <Section id="publish" title="Publish and deploy">
        <SnippetBlock
          method="paxeer-network/hpx/publish.sh"
          args="stages /srv/hpx/artifacts/releases/<id>"
          source="paxeer-network/hpx/publish.sh"
          purpose="Stage paxd, six libwasmvm files, genesis, and config; flip the current symlink only after checksums succeed."
          code={`sudo paxeer-network/hpx/publish.sh`}
        />
        <p className={m3.body}>
          Inputs: <code>build/paxd</code>, wasm-runtime plus <code>wasm/x/wasm/artifacts</code> v152/v155 libs, live config from <code>/root/.paxeer/config</code> or <code>$SRC_CFG</code>. Failed staging never moves <code>current</code>.
        </p>
        <SnippetBlock
          method="paxeer-network/hpx/hosting/deploy.sh"
          args="loopback registry + Nginx + node.hyperpaxeer.com"
          source="paxeer-network/hpx/hosting/"
          purpose="Install the registry executable as a loopback systemd unit and terminate TLS on Nginx."
          code={`sudo paxeer-network/hpx/hosting/deploy.sh

# optional register token
export HPX_REGISTER_TOKEN=<token>
sudo paxeer-network/hpx/hosting/deploy.sh

# trusted mirror only
export HPX_MIRROR=https://<your-mirror>
hpx setup`}
        />
        <p className={m3.body}>
          Workflow <code>Paxeer / HPX Registry</code> builds Linux x86-64 and AArch64 executables and a multi-arch GHCR image when <code>paxeer-network/hpx/registry/</code> changes. Registry state: <code>/srv/hpx/data/registry.json</code>.
        </p>
        <Callout label="Mirrors">
          <code>HPX_MIRROR</code> only for a mirror you already trust. An untrusted mirror can serve a different binary with matching path names.
        </Callout>
      </Section>

      <PageNav prev={{ href: '/interchain', title: 'Interchain' }} />
    </DocsLayout>
  )
}
