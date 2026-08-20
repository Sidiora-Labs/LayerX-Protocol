import { cookies, headers } from "next/headers";

import { AgentsSurface } from "../../../journeys/agents";
import { selectServerShell } from "../../../shell/server";

export const dynamic = "force-dynamic";

export default async function AgentsPage() {
  const [requestHeaders, requestCookies] = await Promise.all([headers(), cookies()]);
  return <AgentsSurface shell={selectServerShell(requestHeaders, requestCookies).shell} />;
}
