import { copyEntry } from "../../../../copy/catalog";
import { ExplorerFrame } from "../../../explorer/components";
import { EvidenceVerifier } from "../../../explorer/verifier";

export default function VerifyPage() {
  return (
    <ExplorerFrame
      title={copyEntry("explorer.verify.title").message}
      description={copyEntry("explorer.verify.body").message}
    >
      <EvidenceVerifier />
    </ExplorerFrame>
  );
}
