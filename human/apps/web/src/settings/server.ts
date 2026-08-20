import { createHash } from "node:crypto";

import { APP_SESSION_COOKIE } from "../proxy";

interface RequestCookies {
  get(name: string): { readonly value: string } | undefined;
}

export function privacyPrincipalScope(headers: Headers, cookies: RequestCookies): string {
  const principalScope = headers.get("x-layerx-principal-scope");
  if (principalScope !== null && /^[A-Za-z0-9_-]{16,128}$/u.test(principalScope)) {
    return principalScope;
  }
  const session = cookies.get(APP_SESSION_COOKIE)?.value ?? "missing-authenticated-session";
  return createHash("sha256")
    .update("layerx-web-privacy-scope/v1\0", "utf8")
    .update(session, "utf8")
    .digest("base64url");
}
