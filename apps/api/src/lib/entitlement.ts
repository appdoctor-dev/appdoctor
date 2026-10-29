import { type Db, subscription } from "@repo/db";
import { desc, eq } from "drizzle-orm";

const DAY_MS = 86_400_000;
/** How long a signed entitlement works offline before the app must check in again. */
const OFFLINE_MS = 7 * DAY_MS;
const ENTITLED_STATUSES = new Set(["active", "trialing"]);

export type EntitlementClaims = {
  v: 1;
  sub: string; // user ID
  plan: string;
  device: string;
  iat: number; // seconds
  exp: number; // seconds
};

/** The user's subscription that currently grants access, if any. past_due keeps access for a grace period. */
export async function currentSubscription(db: Db, userId: string, graceDays: number) {
  const rows = await db
    .select()
    .from(subscription)
    .where(eq(subscription.referenceId, userId))
    .orderBy(desc(subscription.periodEnd));
  const now = Date.now();
  return (
    rows.find(
      (s) =>
        ENTITLED_STATUSES.has(s.status ?? "") ||
        (s.status === "past_due" && s.periodEnd && s.periodEnd.getTime() + graceDays * DAY_MS > now),
    ) ?? null
  );
}

export function entitlementExpiry(periodEnd: Date | null, graceDays: number, now = Date.now()) {
  const offline = now + OFFLINE_MS;
  if (!periodEnd) return offline;
  return Math.min(offline, periodEnd.getTime() + graceDays * DAY_MS);
}

/**
 * Signs `base64url(json).base64url(ed25519)`. The desktop app ships only the
 * public key, so an open-source build can verify entitlements but not mint them.
 */
export async function signEntitlement(signingKey: string, claims: EntitlementClaims) {
  const key = await crypto.subtle.importKey("pkcs8", base64ToBytes(signingKey), { name: "Ed25519" }, false, [
    "sign",
  ]);
  const body = base64url(new TextEncoder().encode(JSON.stringify(claims)));
  const signature = await crypto.subtle.sign("Ed25519", key, new TextEncoder().encode(body));
  return `${body}.${base64url(new Uint8Array(signature))}`;
}

function base64ToBytes(value: string) {
  return Uint8Array.from(atob(value), (c) => c.charCodeAt(0));
}

function base64url(bytes: Uint8Array) {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
}
