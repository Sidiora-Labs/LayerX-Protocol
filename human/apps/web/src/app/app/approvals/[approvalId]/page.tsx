import { notFound } from "next/navigation";

import { ApprovalsJourneyScreen } from "../../../../journeys/approvals";

export default async function ApprovalDetailPage({
  params,
}: Readonly<{ params: Promise<{ approvalId: string }> }>) {
  const { approvalId } = await params;
  if (approvalId.length === 0 || approvalId.length > 128) {
    notFound();
  }
  return <ApprovalsJourneyScreen approvalId={approvalId} />;
}
