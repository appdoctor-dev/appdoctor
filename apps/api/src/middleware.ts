import { createMiddleware } from "hono/factory";
import type { AppEnv } from "./env";

export const requireUser = createMiddleware<AppEnv>(async (c, next) => {
  if (!c.var.user) return c.json({ error: "unauthorized" as const }, 401);
  await next();
});

export const requireAdmin = createMiddleware<AppEnv>(async (c, next) => {
  if (!c.var.user) return c.json({ error: "unauthorized" as const }, 401);
  if (c.var.user.role !== "admin") return c.json({ error: "forbidden" as const }, 403);
  await next();
});
