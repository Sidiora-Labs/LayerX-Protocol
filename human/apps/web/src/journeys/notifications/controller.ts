import type {
  HumanApiClient,
  NotificationPage,
  NotificationSummary,
} from "../../api";
import {
  NOTIFICATIONS_ROUTE,
  presentedNotifications,
  safeDeepLink,
  type PresentedNotification,
} from "./model";

export interface NotificationsOptions {
  readonly client: HumanApiClient;
}

export interface NotificationLanding {
  readonly notification: NotificationSummary;
  readonly href: string;
}

export class Notifications {
  readonly #client: HumanApiClient;

  constructor(options: NotificationsOptions) {
    this.#client = options.client;
  }

  async page(): Promise<NotificationPage> {
    return this.#client.notificationList();
  }

  async archive(): Promise<readonly PresentedNotification[]> {
    return presentedNotifications(await this.page());
  }

  async pendingApprovals(): Promise<number> {
    const page = await this.#client.approvalList();
    const now = Date.now();
    return page.approvals.filter((approval) => {
      const expiry = Date.parse(approval.expires_at);
      return approval.state === "pending" && Number.isFinite(expiry) && expiry > now;
    }).length;
  }

  async open(notification: PresentedNotification): Promise<NotificationLanding> {
    const updated = await this.#client.notificationRead(notification.source.notification_id);
    if (updated.approval_id !== undefined) {
      const approval = await this.#client.approvalGet(updated.approval_id);
      return Object.freeze({
        notification: updated,
        href: `/app/approvals/${encodeURIComponent(approval.approval_id)}`,
      });
    }
    if (updated.journey_id !== undefined) {
      await this.#client.journeyGet(updated.journey_id);
    }
    return Object.freeze({
      notification: updated,
      href: safeDeepLink(updated.deep_link) ?? NOTIFICATIONS_ROUTE,
    });
  }
}
