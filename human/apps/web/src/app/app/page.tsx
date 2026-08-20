import { copyEntry } from "../../../copy/catalog";
import { ScreenCard } from "../../kit";

export default function AppPlanePage() {
  return (
    <ScreenCard
      landmark="section"
      title={copyEntry("navigation.home").message}
      description={copyEntry("app.home.summary").message}
    />
  );
}
