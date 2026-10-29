// Used only by `npm run auth:generate` so the Better Auth CLI can see which
// plugins are enabled and emit their tables as a Drizzle schema.
import { drizzle } from "drizzle-orm/d1";
import Stripe from "stripe";
import { createAuth } from "./src/auth";
import type { Bindings } from "./src/env";

const env = {
  API_URL: "http://localhost:8787",
  WEB_URL: "http://localhost:3000",
  EMAIL_FROM: "AppDoctor <hello@localhost>",
  BETTER_AUTH_SECRET: "schema-generation-only",
  STRIPE_SECRET_KEY: "sk_test_schema_generation_only",
  STRIPE_WEBHOOK_SECRET: "whsec_schema_generation_only",
  LICENSE_SIGNING_KEY: "",
} as Bindings;

export const auth = createAuth(env, drizzle({} as D1Database), new Stripe(env.STRIPE_SECRET_KEY));
