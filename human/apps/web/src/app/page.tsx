import { human_web_app_scaffold } from "./scaffold";

export default function RootPage() {
  return <main data-application={human_web_app_scaffold().application} />;
}
