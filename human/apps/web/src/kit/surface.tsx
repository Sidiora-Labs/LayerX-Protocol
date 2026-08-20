import { Card } from "@layerx/ui";
import type { ReactNode } from "react";

export function ScreenCard({
  title,
  description,
  children,
  landmark = "main",
  dataApplication,
}: Readonly<{
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  landmark?: "main" | "section";
  dataApplication?: string;
}>) {
  const Landmark = landmark;
  return (
    <Card>
      <Landmark data-application={dataApplication} className="flex flex-col gap-2">
        <h1 className="text-xl font-bold text-foreground">{title}</h1>
        {description === undefined ? null : (
          <p className="text-foreground-secondary">{description}</p>
        )}
        {children}
      </Landmark>
    </Card>
  );
}
