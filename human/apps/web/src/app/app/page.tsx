import { Card } from "@layerx/ui";

import { copyEntry } from "../../../copy/catalog";

export default function AppPlanePage() {
  return (
    <Card>
      <section aria-labelledby="app-plane-title">
        <h2 id="app-plane-title">{copyEntry("navigation.home").message}</h2>
        <p>{copyEntry("app.home.summary").message}</p>
      </section>
    </Card>
  );
}
