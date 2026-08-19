import { createHumanApiClient, type HumanApiClient, type HumanApiClientOptions } from "./generated/index.ts";

export * from "./generated/index.ts";

export function humanApi(options: HumanApiClientOptions = {}): HumanApiClient {
  return createHumanApiClient(options);
}
