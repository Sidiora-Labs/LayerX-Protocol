import type { WalletSignRequest } from "../../api/index.ts";

export type WalletHandOffPhase =
  | "idle"
  | "waiting"
  | "approved"
  | "rejected"
  | "cancelled"
  | "unavailable"
  | "failed";

export type WalletSignOutcome =
  | Readonly<{ outcome: "approved"; reference: string }>
  | Readonly<{ outcome: "rejected" }>
  | Readonly<{ outcome: "cancelled" }>
  | Readonly<{ outcome: "unavailable" }>
  | Readonly<{ outcome: "failed" }>;

export interface PaxeerWalletBridge {
  sign(request: WalletSignRequest): Promise<WalletSignOutcome>;
}

export class WalletHandOff {
  readonly #bridge: PaxeerWalletBridge;
  readonly #approvedStages = new Set<string>();
  readonly #opens = new Map<string, number>();
  #phase: WalletHandOffPhase = "idle";
  #reference: string | undefined;
  #attempt = 0;

  constructor(bridge: PaxeerWalletBridge) {
    this.#bridge = bridge;
  }

  get phase(): WalletHandOffPhase {
    return this.#phase;
  }

  get reference(): string | undefined {
    return this.#reference;
  }

  opens(stageId: string): number {
    return this.#opens.get(stageId) ?? 0;
  }

  approved(stageId: string): boolean {
    return this.#approvedStages.has(stageId);
  }

  cancel(): void {
    if (this.#phase === "waiting") {
      this.#phase = "cancelled";
      this.#attempt += 1;
    }
  }

  async open(request: WalletSignRequest): Promise<WalletSignOutcome> {
    if (this.#phase === "waiting") {
      throw new Error("The wallet is already open for a signing moment");
    }
    if (this.#approvedStages.has(request.stage_id)) {
      throw new Error("This signing moment has already been approved");
    }
    this.#attempt += 1;
    const attempt = this.#attempt;
    this.#phase = "waiting";
    this.#opens.set(request.stage_id, this.opens(request.stage_id) + 1);
    let outcome: WalletSignOutcome;
    try {
      outcome = await this.#bridge.sign(request);
    } catch {
      outcome = { outcome: "failed" };
    }
    if (this.#attempt !== attempt) {
      return { outcome: "cancelled" };
    }
    this.#phase = outcome.outcome;
    if (outcome.outcome === "approved") {
      this.#approvedStages.add(request.stage_id);
      this.#reference = outcome.reference;
    }
    return outcome;
  }
}

export interface Eip1193Provider {
  request(args: Readonly<{ method: string; params?: readonly unknown[] }>): Promise<unknown>;
}

const USER_REJECTED_REQUEST = 4001;
const UNAUTHORIZED_REQUEST = 4100;
const PROVIDER_DISCONNECTED = 4900;
const CHAIN_DISCONNECTED = 4901;

export function browserWalletBridge(provider: () => Eip1193Provider | undefined): PaxeerWalletBridge {
  return {
    async sign(request: WalletSignRequest): Promise<WalletSignOutcome> {
      const wallet = provider();
      if (wallet === undefined) {
        return { outcome: "unavailable" };
      }
      try {
        const reference = await wallet.request({
          method: "paxeer_signCustody",
          params: [{ from: request.from_address, data: request.to_sign_base64 }],
        });
        return typeof reference === "string" && reference.length > 0
          ? { outcome: "approved", reference }
          : { outcome: "failed" };
      } catch (error) {
        const code = (error as Readonly<{ code?: unknown }>).code;
        if (code === USER_REJECTED_REQUEST) {
          return { outcome: "cancelled" };
        }
        if (code === UNAUTHORIZED_REQUEST) {
          return { outcome: "rejected" };
        }
        return code === PROVIDER_DISCONNECTED || code === CHAIN_DISCONNECTED
          ? { outcome: "unavailable" }
          : { outcome: "failed" };
      }
    },
  };
}

export function windowWalletProvider(): Eip1193Provider | undefined {
  if (typeof window === "undefined") {
    return undefined;
  }
  const candidate = (window as Readonly<{ paxeer?: unknown }>).paxeer;
  if (typeof candidate !== "object" || candidate === null) {
    return undefined;
  }
  const request = (candidate as Readonly<{ request?: unknown }>).request;
  return typeof request === "function" ? (candidate as Eip1193Provider) : undefined;
}
