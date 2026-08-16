import { Card } from "@layerx/ui";

import { human_web_app_scaffold } from "./scaffold";

export default function RootPage() {
  return (
    <Card>
      <main data-application={human_web_app_scaffold().application} />
    </Card>
  );
}
