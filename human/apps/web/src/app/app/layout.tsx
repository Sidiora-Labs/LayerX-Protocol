import { cookies, headers } from "next/headers";
import { redirect } from "next/navigation";
import type { ReactNode } from "react";

import { AuthenticatedShell } from "../../shell/app-shell";
import { selectServerShell } from "../../shell/server";
import { PrivacyModeProvider } from "../../settings/privacy";
import { privacyPrincipalScope } from "../../settings/server";

export const dynamic = "force-dynamic";

export default async function AppPlaneLayout({ children }: Readonly<{ children: ReactNode }>) {
  const [requestHeaders, requestCookies] = await Promise.all([headers(), cookies()]);
  if (requestHeaders.get("x-layerx-authenticated") !== "1") {
    redirect("/?return_to=%2Fapp");
  }

  return (
    <PrivacyModeProvider principalScope={privacyPrincipalScope(requestHeaders, requestCookies)}>
      <AuthenticatedShell initialSelection={selectServerShell(requestHeaders, requestCookies)}>
        {children}
      </AuthenticatedShell>
    </PrivacyModeProvider>
  );
}
