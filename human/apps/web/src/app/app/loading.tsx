import { copyEntry } from "../../../copy/catalog";
import { PerformanceLoadingCard } from "../../kit";

export default function AppLoading() {
  return (
    <PerformanceLoadingCard plane="app" label={copyEntry("state.loading").message} />
  );
}
