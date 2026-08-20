import { readFile } from "node:fs/promises";
import { FileFulfillmentRepository } from "./fulfillments.js";

const reportBody = await readFile(process.env.LAYERX_RESOURCE_FILE ?? "./resource.json", "utf8");
const fulfillmentDirectory = process.env.LAYERX_FULFILLMENT_DIR ?? "./fulfillments";

export const settlements = [];

// layerx:begin integration
import { SingleProcessWebhookDeliveryStore, mountLayerX } from "@sidiora/layerx-next";
export const layerx = mountLayerX({
  environment: process.env,
  resources: { release: async () => ({ contentType: "application/json", body: reportBody }) },
  fulfillments: new FileFulfillmentRepository(fulfillmentDirectory),
  deliveries: new SingleProcessWebhookDeliveryStore(),
  events: { handle: async (event, deliveryId) => { settlements.push({ deliveryId, event }); } },
});
// layerx:end integration
