import type { ReactNode } from "react";

export const dynamic = "force-static";

export default function ExplorerPlaneLayout({ children }: Readonly<{ children: ReactNode }>) {
  return <>{children}</>;
}
