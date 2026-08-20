import { ScreenCard } from "../kit";
import { human_web_app_scaffold } from "./scaffold";

export default function RootPage() {
  const scaffold = human_web_app_scaffold();
  return (
    <ScreenCard title={scaffold.application} dataApplication={scaffold.application} />
  );
}
