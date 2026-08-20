import { copyEntry } from "../../copy/catalog";
import { PlaneRouteAction, ScreenCard } from "../kit";
import { human_web_app_scaffold } from "./scaffold";

export default function RootPage() {
  const scaffold = human_web_app_scaffold();
  return (
    <ScreenCard title={scaffold.application} dataApplication={scaffold.application}>
      <div className="mt-4">
        <PlaneRouteAction destination="/explorer">
          {copyEntry("action.open_explorer").message}
        </PlaneRouteAction>
      </div>
    </ScreenCard>
  );
}
