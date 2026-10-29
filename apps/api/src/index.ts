import { createDb } from "@repo/db";
import { Hono } from "hono";
import { cors } from "hono/cors";
import { createMiddleware } from "hono/factory";
import Stripe from "stripe";
import { createAuth, createStripe } from "./auth";
import type { AppEnv } from "./env";
import { adminApi } from "./routes/admin";
import { v1 } from "./routes/v1";

const app = new Hono<AppEnv>();

// The website and admin call the API from the browser with the shared session cookie.
app.use(
  "*",
  cors({
    origin: (origin, c) => ([c.env.WEB_URL, c.env.ADMIN_URL].includes(origin) ? origin : null),
    credentials: true,
    allowHeaders: ["Content-Type", "Authorization"],
    allowMethods: ["GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS"],
    exposeHeaders: ["set-auth-token"],
  }),
);

app.use("*", async (c, next) => {
  const db = createDb(c.env.DB);
  const stripe = createStripe(c.env);
  c.set("db", db);
  c.set("stripe", stripe);
  c.set("auth", createAuth(c.env, db, stripe));
  await next();
});

// Sign-in, sessions, device flow, Stripe checkout/portal and the Stripe webhook
// (/api/auth/stripe/webhook) are all served by Better Auth.
app.on(["GET", "POST"], "/api/auth/*", (c) => c.var.auth.handler(c.req.raw));

// Cookie sessions from the browser, bearer tokens from the desktop app.
const loadSession = createMiddleware<AppEnv>(async (c, next) => {
  const session = await c.var.auth.api.getSession({ headers: c.req.raw.headers });
  c.set("user", session?.user ?? null);
  await next();
});
app.use("/v1/*", loadSession);
app.use("/admin/*", loadSession);

app.get("/", (c) => c.json({ name: "appdoctor-api", ok: true }));

// Surface Stripe's message (bad key, missing permission on a restricted key, ...) to the admin UI.
app.onError((err, c) => {
  if (err instanceof Stripe.errors.StripeError) {
    console.error("stripe", err.type, err.message);
    return c.json({ error: "stripe_error" as const, message: err.message }, 502);
  }
  console.error(err);
  return c.json({ error: "internal_error" as const }, 500);
});

const routes = app.route("/v1", v1).route("/admin", adminApi);

export type V1Api = typeof v1;
export type AdminApi = typeof adminApi;
export type ApiRoutes = typeof routes;

export default app;
