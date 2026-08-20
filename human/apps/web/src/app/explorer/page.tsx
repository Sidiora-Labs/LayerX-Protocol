import { Card } from "@layerx/ui";

import { copyEntry } from "../../../copy/catalog";

export default function ExplorerPlanePage() {
  return (
    <Card>
      <main aria-labelledby="explorer-plane-title">
        <h1 id="explorer-plane-title">{copyEntry("explorer.title").message}</h1>
        <p>{copyEntry("explorer.summary").message}</p>
      </main>
    </Card>
  );
}
