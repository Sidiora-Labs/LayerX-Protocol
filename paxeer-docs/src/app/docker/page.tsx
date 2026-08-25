import { DocsLayout } from '@/components/DocsLayout'
import { Callout, FactChips, JumpNav, MethodTable, PageLead, PageNav, Section, SnippetBlock, Subhead, m3 } from '@/components/api/ApiPage'

export default function Docker() {
  return (
    <DocsLayout pageTitle="Docker">
      <PageLead overline="make docker-cluster-start · 192.168.10.0/24 · node0 :8545" source="paxeer-network/docker/docker-compose.yml, paxeer-network/Makefile">
        <p>
          Four-validator localnet on bridge subnet <code>192.168.10.0/24</code>. Node0 publishes EVM JSON-RPC at <code>8545-8546</code> and gRPC at <code>9090-9091</code>. Tendermint ports are the host range <code>26656-26658</code>.
        </p>
        <p>
          Image <code>pax-chain/localnode</code> from <code>paxeer-network/Dockerfile</code>. Platform defaults to <code>linux/amd64</code> unless <code>DOCKER_PLATFORM</code> is set.
        </p>
      </PageLead>

      <FactChips
        items={[
          { label: 'Node0 EVM', value: '8545-8546' },
          { label: 'Node0 gRPC', value: '9090-9091' },
          { label: 'Subnet', value: '192.168.10.0/24' },
          { label: 'Prometheus host', value: ':9099' },
        ]}
      />

      <JumpNav
        items={[
          { id: 'prereq', label: 'Prerequisites' },
          { id: 'cluster', label: 'Four-node cluster' },
          { id: 'ports', label: 'Ports' },
          { id: 'env', label: 'Env' },
          { id: 'compose', label: 'Compose files' },
          { id: 'monitor', label: 'Monitoring' },
          { id: 'statesync', label: 'State sync' },
        ]}
      />

      <Section id="prereq" title="Prerequisites">
        <p className={m3.body}>
          macOS: Docker Desktop from docs.docker.com. Ubuntu: Docker Engine plus Compose from the Docker install docs.
        </p>
      </Section>

      <Section id="cluster" title="Four-node cluster">
        <SnippetBlock
          method="make docker-cluster-start"
          args="build-docker-node, then four pax-node-* containers"
          source="paxeer-network/Makefile"
          purpose="Build the localnode image and start the four-validator cluster."
          code={`make docker-cluster-start
make docker-cluster-start-skipbuild`}
        />
        <p className={m3.body}>
          Genesis and logs land under <code>build/generated/</code>. Single-node (<code>make build-docker-node && make run-local-node</code>) is for minimal checks only.
        </p>
        <SnippetBlock
          method="tail / docker exec"
          args="pax-node-0"
          source="paxeer-network/docker/"
          purpose="Follow node0 logs or open a shell in the container."
          code={`tail -f build/generated/logs/paxd-0.log
ls -l build/generated/logs/
docker ps -a
docker exec -it pax-node-0 /bin/bash`}
        />
      </Section>

      <Section id="ports" title="Host ports">
        <p className={m3.body}>
          Published mappings from <code>paxeer-network/docker/docker-compose.yml</code>. Container Tendermint is always <code>26656-26658</code>; host ranges shift per node.
        </p>
        <MethodTable
          columns={['Node', 'Tendermint host', 'gRPC host', 'EVM host', 'IP']}
          rows={[
            ['node0 / pax-node-0', '26656-26658', '9090-9091', '8545-8546', '192.168.10.10'],
            ['node1 / pax-node-1', '26659-26661', '9092-9093', '8547-8548', '192.168.10.11'],
            ['node2 / pax-node-2', '26662-26664', '9094-9095', '8549-8550', '192.168.10.12'],
            ['node3 / pax-node-3', '26665-26667', '9096-9097', '8551-8552', '192.168.10.13'],
          ]}
        />
        <Callout label="REST :1317">
          The SDK default REST address is <code>tcp://0.0.0.0:1317</code>. This compose file does not publish 1317 on the host.
        </Callout>
      </Section>

      <Section id="env" title="Environment">
        <MethodTable
          columns={['Variable', 'Purpose']}
          rows={[
            ['NUM_ACCOUNTS', 'Test accounts to create'],
            ['SKIP_BUILD', 'Skip binary rebuild'],
            ['INVARIANT_CHECK_INTERVAL', 'Invariant check frequency'],
            ['UPGRADE_VERSION_LIST', 'Upgrade version schedule'],
            ['MOCK_BALANCES', 'Mock balances (node0)'],
            ['GIGA_EXECUTOR', 'Giga executor backend'],
            ['GIGA_OCC', 'Optimistic concurrency'],
            ['RECEIPT_BACKEND', 'Receipt storage backend'],
            ['AUTOBAHN', 'Autobahn consensus path'],
            ['GIGA_STORAGE', 'Giga storage engine'],
            ['GIGA_MIGRATE_FROM_MEMIAVL', 'Migrate MEMIAVL → Giga'],
            ['GIGA_FLATKV_ONLY', 'Flat KV only'],
          ]}
        />
      </Section>

      <Section id="compose" title="Compose files">
        <MethodTable
          columns={['File', 'Purpose']}
          rows={[
            ['docker/docker-compose.yml', 'Four-node cluster'],
            ['docker/docker-compose.monitoring.yml', 'Prometheus + Grafana overlay'],
            ['docker/docker-compose.giga-mixed.yml', 'Mixed Giga / legacy storage'],
          ]}
        />
        <p className={m3.body}>
          Mounts: <code>$PROJECT_HOME</code> → <code>/pax-protocol/pax-chain</code>, <code>$GO_PKG_PATH/mod</code> → <code>/root/go/pkg/mod</code>, <code>$GOCACHE</code> → <code>/root/.cache/go-build</code>.
        </p>
        <Subhead>Local node scripts</Subhead>
        <p className={m3.body}>
          <code>docker/localnode/scripts/</code>: <code>step0_build.sh</code>, <code>step1_configure_init.sh</code>, <code>step2_genesis.sh</code>, <code>step3_add_validator_to_genesis.sh</code>, <code>step4_config_override.sh</code>, <code>step5_start_pax.sh</code>, <code>deploy.sh</code>.
        </p>
        <Subhead>RPC node scripts</Subhead>
        <p className={m3.body}>
          <code>docker/rpcnode/scripts/</code>: <code>step0_build.sh</code>, <code>step1_configure_init.sh</code>, <code>step2_start_pax.sh</code>, <code>deploy.sh</code>.
        </p>
      </Section>

      <Section id="monitor" title="Monitoring">
        <SnippetBlock
          method="make docker-cluster-start-monitoring"
          args="Prometheus host :9099, Grafana :3000"
          source="paxeer-network/docker/docker-compose.monitoring.yml"
          purpose="Start the four-node cluster plus Prometheus (host 9099) and Grafana (host 3000)."
          code={`make docker-cluster-start-monitoring
make docker-cluster-stop-monitoring

./docker/monitornode/scripts/start-prometheus.sh
./docker/monitornode/scripts/start-grafana.sh
./docker/monitornode/scripts/stop-prometheus.sh
./docker/monitornode/scripts/stop-grafana.sh`}
        />
        <MethodTable
          columns={['UI', 'Host']}
          rows={[
            ['Grafana', 'http://localhost:3000 (admin / admin)'],
            ['Prometheus', 'http://localhost:9099'],
          ]}
        />
        <p className={m3.body}>
          Prometheus is published as <code>9099:9090</code> so it does not collide with node0 gRPC on host 9090.
        </p>
      </Section>

      <Section id="statesync" title="State-sync RPC node">
        <SnippetBlock
          method="make run-rpc-node"
          args="requires a running 4-node cluster"
          source="paxeer-network/Makefile, docker/rpcnode/scripts/"
          purpose="Add a state-sync RPC node after the cluster is past the configured height."
          code={`make docker-cluster-start
paxd status | jq
make run-rpc-node`}
        />
        <p className={m3.body}>
          Fast iteration: edit under <code>paxeer-network/</code>, <code>make build-docker-node</code>, <code>make docker-cluster-start</code>. No sibling-repo <code>go.mod</code> replace is required.
        </p>
      </Section>

      <PageNav
        prev={{ href: '/contracts', title: 'Contracts' }}
        next={{ href: '/sdk', title: 'SDK' }}
      />
    </DocsLayout>
  )
}
