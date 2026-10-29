import { createDb } from "@repo/db";
import { WorkerEntrypoint } from "cloudflare:workers";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { createMiddleware } from "hono/factory";
import type { ErrorHandler } from "hono";
import Stripe from "stripe";
import { createAuth, createStripe } from "./auth";
import type { AppEnv, Bindings } from "./env";
import { adminApi } from "./routes/admin";
import { v1 } from "./routes/v1";

const withContext = createMiddleware<AppEnv>(async (c, next) => {
  const db = createDb(c.env.DB);
  const stripe = createStripe(c.env);
  c.set("db", db);
  c.set("stripe", stripe);
  c.set("auth", createAuth(c.env, db, stripe));
  await next();
});

// Surface Stripe's message (bad key, missing permission on a restricted key, ...) to the UI.
const onError: ErrorHandler<AppEnv> = (err, c) => {
  if (err instanceof Stripe.errors.StripeError) {
    console.error("stripe", err.type, err.message);
    return c.json({ error: "stripe_error" as const, message: err.message }, 502);
  }
  console.error(err);
  return c.json({ error: "internal_error" as const }, 500);
};

/* Public API: the website (browser, cookie session) and the desktop app (bearer token). */

const app = new Hono<AppEnv>();

app.use(
  "*",
  cors({
    origin: (origin, c) => (origin === c.env.WEB_URL ? origin : null),
    credentials: true,
    allowHeaders: ["Content-Type", "Authorization"],
    allowMethods: ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"],
    exposeHeaders: ["set-auth-token"],
  }),
);
app.use("*", withContext);

// Sign-in, sessions, device flow, Stripe checkout/portal and the Stripe webhook
// (/api/auth/stripe/webhook) are all served by Better Auth.
app.on(["GET", "POST"], "/api/auth/*", (c) => c.var.auth.handler(c.req.raw));

app.use(
  "/v1/*",
  createMiddleware<AppEnv>(async (c, next) => {
    const session = await c.var.auth.api.getSession({ headers: c.req.raw.headers });
    c.set("user", session?.user ?? null);
    await next();
  }),
);

app.get("/", (c) => c.json({ name: "appdoctor-api", ok: true }));
app.route("/v1", v1);
app.onError(onError);

/* Admin API: never routed from the internet, only through the AdminEntrypoint binding. */

const admin = new Hono<AppEnv>();
admin.use("*", withContext);
admin.use(
  "*",
  createMiddleware<AppEnv>(async (c, next) => {
    // The admin Worker verified this identity with Cloudflare Access before calling.
    const email = c.req.header("x-admin-email");
    if (!email) return c.json({ error: "forbidden" as const }, 403);
    c.set("adminEmail", email);
    await next();
  }),
);
admin.route("/admin", adminApi);
admin.onError(onError);

/** Bound by the admin Worker as `API` (`entrypoint: "AdminEntrypoint"` in its wrangler.jsonc). */
export class AdminEntrypoint extends WorkerEntrypoint<Bindings> {
  override fetch(request: Request) {
    return admin.fetch(request, this.env, this.ctx);
  }
}

export type V1Api = typeof v1;
export type AdminApi = typeof adminApi;

export default app;
