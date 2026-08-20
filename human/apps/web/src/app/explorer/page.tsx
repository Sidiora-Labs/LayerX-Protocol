import { copyEntry } from "../../../copy/catalog";
import { ScreenCard } from "../../kit";

export default function ExplorerPlanePage() {
  return (
    <ScreenCard
      title={copyEntry("explorer.title").message}
      description={copyEntry("explorer.summary").message}
    />
  );
}
