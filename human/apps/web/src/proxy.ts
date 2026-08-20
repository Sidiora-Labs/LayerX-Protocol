import { type NextRequest, NextResponse } from "next/server.js";

import { APP_SESSION_COOKIE } from "./auth/session.ts";
import { appContentSecurityPolicy } from "./security/csp.ts";

function nonce(): string {
  const bytes = new Uint8Array(18);
  crypto.getRandomValues(bytes);
  return btoa(String.fromCharCode(...bytes));
}

export function proxy(request: NextRequest) {
  const requestNonce = nonce();
  const policy = appContentSecurityPolicy(requestNonce, process.env.NODE_ENV === "development");
  const requestHeaders = new Headers(request.headers);
  requestHeaders.set("content-security-policy", policy);
  requestHeaders.set("x-nonce", requestNonce);

  const session = request.cookies.get(APP_SESSION_COOKIE)?.value;
  if (session === undefined || session.length === 0) {
    const destination = new URL("/", request.url);
    destination.searchParams.set("return_to", `${request.nextUrl.pathname}${request.nextUrl.search}`);
    const response = NextResponse.redirect(destination);
    response.headers.set("content-security-policy", policy);
    return response;
  }

  requestHeaders.set("x-layerx-authenticated", "1");
  const response = NextResponse.next({ request: { headers: requestHeaders } });
  response.headers.set("content-security-policy", policy);
  response.headers.set("accept-ch", "Viewport-Width");
  response.headers.set("critical-ch", "Viewport-Width");
  response.headers.set("referrer-policy", "same-origin");
  response.headers.set("x-content-type-options", "nosniff");
  response.headers.set("permissions-policy", "camera=(), microphone=(), geolocation=()");
  return response;
}

export const config = {
  matcher: ["/app/:path*"],
};
