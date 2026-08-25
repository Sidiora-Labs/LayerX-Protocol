import { DocsLayout } from '@/components/DocsLayout'
import { Callout, FactChips, JumpNav, MethodTable, PageLead, PageNav, Section, SnippetBlock, Subhead, m3 } from '@/components/api/ApiPage'
import Link from 'next/link'

export default function Contracts() {
  return (
    <DocsLayout pageTitle="Contracts">
      <PageLead overline="paxeer-network/contracts/ · contracts/ · Solidity 0.8.28" source="paxeer-network/contracts/, contracts/ at repository root">
        <p>
          Two trees. Paxeer-native utilities live under <code>paxeer-network/contracts/</code>. LayerX settlement — custody, checkpoints, guarantor bonds, withdrawals, emergency exit — lives at repo-root <code>contracts/</code> and deploys <em>on</em> Paxeer (chain ID 125).
        </p>
        <p>
          PAX is gas. LayerX activities charge 5,000 µUSDX, not zero. No public LayerX RPC is documented here. Do not invent deployment addresses; none are published in the tree.
        </p>
      </PageLead>

      <FactChips
        items={[
          { label: 'Paxeer-native', value: 'paxeer-network/contracts/' },
          { label: 'LayerX settlement', value: 'contracts/' },
          { label: 'Solidity', value: '0.8.28 / Prague' },
          { label: 'Local EVM', value: 'http://127.0.0.1:8545' },
        ]}
      />

      <JumpNav
        items={[
          { id: 'trees', label: 'Two trees' },
          { id: 'native', label: 'Paxeer-native' },
          { id: 'precompiles', label: 'Precompile interfaces' },
          { id: 'build', label: 'Build' },
          { id: 'layerx', label: 'LayerX settlement' },
        ]}
      />

      <Section id="trees" title="Two trees">
        <MethodTable
          columns={['Path', 'Purpose', 'Tooling']}
          rows={[
            ['paxeer-network/contracts/', 'WPAX, pointer contracts, precompile interfaces, testers', 'Foundry + Hardhat'],
            ['contracts/', 'LayerX custody, checkpoints, bonds, claims, emergency exit', 'Solidity 0.8.28'],
          ]}
        />
      </Section>

      <Section id="native" title="Paxeer-native">
        <p className={m3.body}>
          Chain-side Solidity under <code>paxeer-network/contracts/src/</code>.
        </p>
        <MethodTable
          columns={['Contract', 'Purpose']}
          rows={[
            ['WPAX.sol', 'ERC-20 wrapper for native PAX'],
            ['CW20ERC20Pointer.sol', 'EVM pointer for CW20'],
            ['CW721ERC721Pointer.sol', 'EVM pointer for CW721'],
            ['CW1155ERC1155Pointer.sol', 'EVM pointer for CW1155'],
            ['NativePaxTokensERC20.sol', 'ERC-20 view of native Cosmos tokens'],
          ]}
        />
        <Subhead>Testers</Subhead>
        <p className={m3.body}>
          <code>EVMCompatibilityTester.sol</code>, <code>TransientStorageTester.sol</code>, <code>SelfDestructTester.sol</code>, <code>SnapshotRevertTester.sol</code>, <code>SstoreGasTest.sol</code>, <code>ProxySwapTester.sol</code>, <code>MultiHopSwapTester.sol</code>, <code>MultiSender.sol</code>, <code>BatchCallAndSponsor.sol</code>, <code>Box.sol</code>, <code>BoxV2.sol</code>, <code>TestToken.sol</code>, <code>ERC721.sol</code>, <code>ERC1155.sol</code>.
        </p>
      </Section>

      <Section id="precompiles" title="Precompile interfaces">
        <p className={m3.body}>
          Addresses are the constants in <code>paxeer-network/contracts/src/precompiles/</code> (same values in <code>paxeer-network/precompiles/</code>).
        </p>
        <MethodTable
          columns={['Interface', 'Address', 'Purpose']}
          rows={[
            ['IAddr.sol', '0x0000000000000000000000000000000000001004', 'EVM ↔ Cosmos address map'],
            ['IBank.sol', '0x0000000000000000000000000000000000001001', 'Native bank send and balance'],
            ['IWasmd.sol', '0x0000000000000000000000000000000000001002', 'CosmWasm calls from EVM'],
            ['IJson.sol', '0x0000000000000000000000000000000000001003', 'JSON extract helpers'],
          ]}
        />
      </Section>

      <Section id="build" title="Build">
        <SnippetBlock
          method="forge build / npx hardhat compile"
          args="Solidity 0.8.28, evmVersion prague"
          source="paxeer-network/contracts/README.md, paxeer-network/contracts/hardhat.config.js"
          purpose="Compile Paxeer-native contracts with Foundry or Hardhat against local :8545."
          code={`cd paxeer-network/contracts
forge install
forge build

npm install
npx hardhat compile
npx hardhat test --network paxlocal`}
        />
        <MethodTable
          columns={['Hardhat network', 'URL']}
          rows={[
            ['paxlocal', 'http://127.0.0.1:8545'],
            ['devnet', 'https://evm-rpc.arctic-1.paxnetwork.io/'],
          ]}
        />
        <p className={m3.body}>
          URLs are the <code>networks</code> entries in <code>paxeer-network/contracts/hardhat.config.js</code>. That file is not a LayerX RPC.
        </p>
        <Subhead>Pointer bytecode</Subhead>
        <p className={m3.body}>
          After a pointer change: <code>forge build</code>, copy bytecode from <code>contracts/out/</code> into the matching <code>.bin</code> under <code>modules/evm/contracts/</code>, restart <code>paxd</code>.
        </p>
      </Section>

      <Section id="layerx" title="LayerX settlement">
        <p className={m3.body}>
          Root <code>contracts/</code> is owned by LayerX governance and deployed on Paxeer. Paxeer validators do not operate these contracts.
        </p>
        <MethodTable
          columns={['Contract', 'Purpose']}
          rows={[
            ['CheckpointRegistry.sol', 'Checkpoint settlement: state roots, heights, timestamps'],
            ['LayerXCustody.sol', 'Custody of USDL backing USDX'],
            ['GuarantorBond.sol', 'Guarantor collateral and slashing'],
            ['WithdrawalClaims.sol', 'Exit claims against checkpoint merkle proofs'],
            ['EmergencyExit.sol', 'Last-resort exit if the LayerX sequencer halts'],
          ]}
        />
        <p className={m3.body}>
          Supporting dirs in that tree: <code>challenge/</code>, <code>custody/</code>, <code>governance/</code>, <code>interfaces/</code>, <code>libraries/</code>, <code>manager/</code>, <code>security/</code>, <code>storage/</code>, <code>config/</code>, <code>deployment/</code>.
        </p>
        <Callout label="Addresses">
          No mainnet or testnet deployment addresses are checked in. Do not invent them. Look at <code>contracts/deployment/</code> only for scripts and helpers that exist in git.
        </Callout>
        <MethodTable
          columns={['', 'Paxeer-native', 'LayerX settlement']}
          rows={[
            ['Path', 'paxeer-network/contracts/', 'contracts/'],
            ['Role', 'Pointers, WPAX, testers', 'Checkpoints, custody, exits'],
            ['Deploy', 'Chain utilities and precompile interfaces', 'Ordinary contracts on Paxeer'],
            ['Control', 'Paxeer chain', 'LayerX governance'],
          ]}
        />
      </Section>

      <PageNav
        prev={{ href: '/rest-grpc', title: 'REST & gRPC' }}
        next={{ href: '/docker', title: 'Docker' }}
      />
    </DocsLayout>
  )
}
