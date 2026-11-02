import type { Bindings } from "../env";

/**
 * Google Search Console: the searches that showed the site and brought people
 * to it. Search engines no longer pass the query along with a visit, so this
 * is the only place it can come from. Needs GSC_SERVICE_ACCOUNT, a Google Cloud
 * service account's JSON key, with that account added as a user of the Search
 * Console property (apps/api/scripts/search-console.sh).
 */

const DAY_MS = 86_400_000;
const SCOPE = "https://www.googleapis.com/auth/webmasters.readonly";

type ServiceAccount = { client_email: string; private_key: string };
type Row = { keys: string[]; clicks: number; impressions: number; ctr: number; position: number };

export async function searchQueries(env: Bindings, days: number) {
  if (!env.GSC_SERVICE_ACCOUNT) return { configured: false as const };
  const account = JSON.parse(env.GSC_SERVICE_ACCOUNT) as ServiceAccount;
  const site = env.GSC_SITE ?? "sc-domain:appdoctor.so";
  try {
    const token = await accessToken(account);
    const date = (ms: number) => new Date(ms).toISOString().slice(0, 10);
    const query = async (dimensions: string[]) => {
      const res = await fetch(
        `https://www.googleapis.com/webmasters/v3/sites/${encodeURIComponent(site)}/searchAnalytics/query`,
        {
          method: "POST",
          headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
          body: JSON.stringify({
            startDate: date(Date.now() - days * DAY_MS),
            endDate: date(Date.now()),
            dimensions,
            rowLimit: 100,
            dataState: "all",
          }),
        },
      );
      const body = (await res.json()) as { rows?: Row[]; error?: { message: string } };
      if (!res.ok) throw new Error(body.error?.message ?? `Search Console answered ${res.status}`);
      return body.rows ?? [];
    };
    const [queries, pages] = await Promise.all([query(["query"]), query(["query", "page"])]);
    const stats = (r: Row) => ({ clicks: r.clicks, impressions: r.impressions, ctr: r.ctr, position: r.position });
    return {
      configured: true as const,
      site,
      account: account.client_email,
      queries: queries.map((r) => ({ query: r.keys[0]!, ...stats(r) })),
      // Which page each search landed on.
      pages: pages.map((r) => ({ query: r.keys[0]!, page: new URL(r.keys[1]!).pathname, ...stats(r) })),
    };
  } catch (error) {
    return {
      configured: true as const,
      site,
      account: account.client_email,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

/** A short-lived OAuth token from a service account's key (Google's JWT bearer flow). */
async function accessToken(account: ServiceAccount) {
  const now = Math.floor(Date.now() / 1000);
  const json = (value: object) => base64url(new TextEncoder().encode(JSON.stringify(value)));
  const unsigned = `${json({ alg: "RS256", typ: "JWT" })}.${json({
    iss: account.client_email,
    scope: SCOPE,
    aud: "https://oauth2.googleapis.com/token",
    iat: now,
    exp: now + 3600,
  })}`;
  const der = Uint8Array.from(atob(account.private_key.replace(/-----[^-]+-----|\s/g, "")), (ch) => ch.charCodeAt(0));
  const key = await crypto.subtle.importKey("pkcs8", der, { name: "RSASSA-PKCS1-v1_5", hash: "SHA-256" }, false, ["sign"]);
  const signature = await crypto.subtle.sign("RSASSA-PKCS1-v1_5", key, new TextEncoder().encode(unsigned));
  const res = await fetch("https://oauth2.googleapis.com/token", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      grant_type: "urn:ietf:params:oauth:grant-type:jwt-bearer",
      assertion: `${unsigned}.${base64url(new Uint8Array(signature))}`,
    }),
  });
  const body = (await res.json()) as { access_token?: string; error_description?: string };
  if (!body.access_token) throw new Error(body.error_description ?? "Google didn't issue a token");
  return body.access_token;
}

function base64url(bytes: Uint8Array) {
  return btoa(String.fromCharCode(...bytes))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}
