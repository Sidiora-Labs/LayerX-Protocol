import { cookies, headers } from "next/headers";

import { AgentCreateJourney } from "../../../../journeys/agents";
import { selectServerShell } from "../../../../shell/server";

export const dynamic = "force-dynamic";

export default async function AgentCreatePage() {
  const [requestHeaders, requestCookies] = await Promise.all([headers(), cookies()]);
  return <AgentCreateJourney shell={selectServerShell(requestHeaders, requestCookies).shell} />;
}
