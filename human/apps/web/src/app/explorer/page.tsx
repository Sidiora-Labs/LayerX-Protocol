import { copyEntry } from "../../../copy/catalog";
import { PlaneRouteAction, ScreenCard } from "../../kit";

export default function ExplorerPlanePage() {
  return (
    <ScreenCard
      title={copyEntry("explorer.title").message}
      description={copyEntry("explorer.summary").message}
    >
      <div className="mt-4">
        <PlaneRouteAction destination="/app">
          {copyEntry("action.open_app").message}
        </PlaneRouteAction>
      </div>
    </ScreenCard>
  );
}
