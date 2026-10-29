import type { Db } from "@repo/db";
import type Stripe from "stripe";
import type { Auth } from "./auth";

// Declared explicitly instead of using Wrangler's global `Env`, because the web
// and admin apps type-check these routes through the Hono RPC client and their
// own global `Env` is a different shape.
export type Bindings = {
  DB: D1Database;
  RELEASES: R2Bucket;

  API_URL: string;
  WEB_URL: string;
  /** e.g. ".appdoctor.so" so the website and the API share one session in production. */
  COOKIE_DOMAIN?: string;
  EMAIL_FROM: string;
  STRIPE_AUTOMATIC_TAX?: string;
  LICENSE_GRACE_DAYS?: string;

  // Secrets (.dev.vars locally, `wrangler secret put` in production)
  BETTER_AUTH_SECRET: string;
  STRIPE_SECRET_KEY: string;
  STRIPE_WEBHOOK_SECRET: string;
  /** Base64 PKCS#8 Ed25519 key; `npm run keys:generate -w @repo/api`. */
  LICENSE_SIGNING_KEY: string;
  RESEND_API_KEY?: string;
  GOOGLE_CLIENT_ID?: string;
  GOOGLE_CLIENT_SECRET?: string;
};

type SessionUser = Auth["$Infer"]["Session"]["user"];

export type AppEnv = {
  Bindings: Bindings;
  Variables: {
    db: Db;
    auth: Auth;
    stripe: Stripe;
    user: SessionUser | null;
    /** Set on admin requests: the Cloudflare Access identity the admin Worker verified. */
    adminEmail: string;
  };
};
