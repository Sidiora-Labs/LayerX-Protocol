import { cookies, headers } from "next/headers";

import { AgentDetailScreen } from "../../../../journeys/agents";
import { selectServerShell } from "../../../../shell/server";

export const dynamic = "force-dynamic";

export default async function AgentDetailPage({
  params,
}: Readonly<{ params: Promise<{ agentId: string }> }>) {
  const [requestHeaders, requestCookies, { agentId }] = await Promise.all([
    headers(),
    cookies(),
    params,
  ]);
  return (
    <AgentDetailScreen
      shell={selectServerShell(requestHeaders, requestCookies).shell}
      agentId={agentId}
    />
  );
}
