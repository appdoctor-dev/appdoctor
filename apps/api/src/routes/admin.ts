import { zValidator } from "@hono/zod-validator";
import { activations, alerts, plans, subscription, user } from "@repo/db";
import { and, asc, count, desc, eq, gte, inArray, isNull, like, or, type SQL } from "drizzle-orm";
import { Hono } from "hono";
import { z } from "zod";
import type { AppEnv } from "../env";
import { siteAnalytics } from "../lib/analytics";
import { searchQueries } from "../lib/search-console";

const ENTITLED = ["active", "trialing", "past_due"];
const DAY_MS = 86_400_000;

const slug = z
  .string()
  .min(2)
  .max(32)
  .regex(/^[a-z0-9-]+$/, "lowercase letters, numbers and dashes");
const amount = z.number().int().positive(); // minor units

const createPlan = z.object({
  id: slug,
  name: z.string().min(1).max(60),
  description: z.string().max(300).default(""),
  features: z.array(z.string().max(120)).max(20).default([]),
  currency: z.string().length(3).toLowerCase().default("usd"),
  monthlyAmount: amount,
  yearlyAmount: amount.nullable().default(null),
  maxDevices: z.number().int().min(1).max(50).default(2),
  trialDays: z.number().int().min(0).max(90).default(0),
  sort: z.number().int().default(0),
});

const updatePlan = createPlan
  .pick({ name: true, description: true, features: true, maxDevices: true, trialDays: true, sort: true })
  .partial()
  .extend({ active: z.boolean().optional() });

const page = {
  limit: z.coerce.number().int().min(1).max(200).default(50),
  offset: z.coerce.number().int().min(0).default(0),
  q: z.string().max(100).optional(),
};

const period = z.object({ days: z.coerce.number().int().min(1).max(400).default(30) });

/**
 * Served only through the AdminEntrypoint service binding (see index.ts), so
 * nothing here is reachable from the internet. The admin Worker has already
 * checked the caller's Cloudflare Access identity.
 */
export const adminApi = new Hono<AppEnv>()

  .get("/stats", async (c) => {
    const db = c.var.db;
    const [subs, planRows, [users], [recentSignups], [openAlerts]] = await Promise.all([
      db
        .select({
          status: subscription.status,
          plan: subscription.plan,
          interval: subscription.billingInterval,
          canceledAt: subscription.canceledAt,
        })
        .from(subscription),
      db.select().from(plans),
      db.select({ n: count() }).from(user),
      db
        .select({ n: count() })
        .from(user)
        .where(gte(user.createdAt, new Date(Date.now() - 30 * DAY_MS))),
      db.select({ n: count() }).from(alerts).where(isNull(alerts.resolvedAt)),
    ]);

    const planById = new Map(planRows.map((p) => [p.id, p]));
    const byStatus: Record<string, number> = {};
    const byPlan: Record<string, number> = {};
    let mrr = 0;
    for (const s of subs) {
      const status = s.status ?? "unknown";
      byStatus[status] = (byStatus[status] ?? 0) + 1;
      if (!ENTITLED.includes(status)) continue;
      byPlan[s.plan] = (byPlan[s.plan] ?? 0) + 1;
      // Estimated from current plan prices; grandfathered subscribers may pay less.
      const plan = planById.get(s.plan);
      if (status === "trialing" || !plan) continue;
      mrr += s.interval === "year" ? (plan.yearlyAmount ?? 0) / 12 : (plan.monthlyAmount ?? 0);
    }
    const canceledLast30d = subs.filter(
      (s) => s.canceledAt && s.canceledAt.getTime() > Date.now() - 30 * DAY_MS,
    ).length;

    return c.json({
      active: byStatus.active ?? 0,
      trialing: byStatus.trialing ?? 0,
      pastDue: byStatus.past_due ?? 0,
      canceledLast30d,
      estimatedMrr: Math.round(mrr),
      currency: planRows[0]?.currency ?? "usd",
      byPlan: Object.entries(byPlan).map(([plan, n]) => ({ plan, name: planById.get(plan)?.name ?? plan, count: n })),
      users: users?.n ?? 0,
      signupsLast30d: recentSignups?.n ?? 0,
      openAlerts: openAlerts?.n ?? 0,
    });
  })

  .get(
    "/subscriptions",
    zValidator("query", z.object({ ...page, status: z.enum(["entitled", "all", "active", "trialing", "past_due", "canceled", "incomplete"]).default("entitled") })),
    async (c) => {
      const { status, q, limit, offset } = c.req.valid("query");
      const filters: (SQL | undefined)[] = [
        status === "entitled"
          ? inArray(subscription.status, ENTITLED)
          : status === "all"
            ? undefined
            : eq(subscription.status, status),
        q ? or(like(user.email, `%${q}%`), like(user.name, `%${q}%`)) : undefined,
      ];
      const where = and(...filters);
      const [rows, [total]] = await Promise.all([
        c.var.db
          .select({
            id: subscription.id,
            userId: user.id,
            email: user.email,
            name: user.name,
            plan: subscription.plan,
            planName: plans.name,
            status: subscription.status,
            interval: subscription.billingInterval,
            periodEnd: subscription.periodEnd,
            cancelAtPeriodEnd: subscription.cancelAtPeriodEnd,
            trialEnd: subscription.trialEnd,
            stripeCustomerId: subscription.stripeCustomerId,
            stripeSubscriptionId: subscription.stripeSubscriptionId,
          })
          .from(subscription)
          .innerJoin(user, eq(user.id, subscription.referenceId))
          .leftJoin(plans, eq(plans.id, subscription.plan))
          .where(where)
          .orderBy(desc(subscription.periodStart))
          .limit(limit)
          .offset(offset),
        c.var.db
          .select({ n: count() })
          .from(subscription)
          .innerJoin(user, eq(user.id, subscription.referenceId))
          .where(where),
      ]);
      return c.json({ rows, total: total?.n ?? 0 });
    },
  )

  .get("/users", zValidator("query", z.object(page)), async (c) => {
    const { q, limit, offset } = c.req.valid("query");
    const where = q ? or(like(user.email, `%${q}%`), like(user.name, `%${q}%`)) : undefined;
    const [rows, [total]] = await Promise.all([
      c.var.db
        .select({
          id: user.id,
          email: user.email,
          name: user.name,
          createdAt: user.createdAt,
          stripeCustomerId: user.stripeCustomerId,
        })
        .from(user)
        .where(where)
        .orderBy(desc(user.createdAt))
        .limit(limit)
        .offset(offset),
      c.var.db.select({ n: count() }).from(user).where(where),
    ]);
    return c.json({ rows, total: total?.n ?? 0 });
  })

  .get("/users/:id", async (c) => {
    const id = c.req.param("id");
    const found = await c.var.db.query.user.findFirst({ where: eq(user.id, id) });
    if (!found) return c.json({ error: "not_found" as const }, 404);
    const [subs, devices, userAlerts] = await Promise.all([
      c.var.db.select().from(subscription).where(eq(subscription.referenceId, id)).orderBy(desc(subscription.periodStart)),
      c.var.db.select().from(activations).where(eq(activations.userId, id)).orderBy(desc(activations.lastSeenAt)),
      found.stripeCustomerId
        ? c.var.db.select().from(alerts).where(eq(alerts.stripeCustomerId, found.stripeCustomerId)).orderBy(desc(alerts.createdAt))
        : Promise.resolve([]),
    ]);
    return c.json({ user: found, subscriptions: subs, devices, alerts: userAlerts });
  })

  .delete("/users/:id/devices/:deviceId", async (c) => {
    await c.var.db
      .delete(activations)
      .where(and(eq(activations.userId, c.req.param("id")), eq(activations.deviceId, c.req.param("deviceId"))));
    return c.json({ ok: true });
  })

  .get("/plans", async (c) => {
    return c.json(await c.var.db.select().from(plans).orderBy(asc(plans.sort)));
  })

  .post("/plans", zValidator("json", createPlan), async (c) => {
    const body = c.req.valid("json");
    const stripe = c.var.stripe;
    if (await c.var.db.query.plans.findFirst({ where: eq(plans.id, body.id) })) {
      return c.json({ error: "plan_exists" as const }, 409);
    }

    // One Product per plan; monthly and yearly are Prices on it.
    const product = await stripe.products.create({
      name: body.name,
      description: body.description || undefined,
      metadata: { plan_id: body.id },
    });
    const monthly = await stripe.prices.create({
      product: product.id,
      currency: body.currency,
      unit_amount: body.monthlyAmount,
      recurring: { interval: "month" },
    });
    const yearly = body.yearlyAmount
      ? await stripe.prices.create({
          product: product.id,
          currency: body.currency,
          unit_amount: body.yearlyAmount,
          recurring: { interval: "year" },
        })
      : null;
    await stripe.products.update(product.id, { default_price: monthly.id });

    const [created] = await c.var.db
      .insert(plans)
      .values({
        ...body,
        stripeProductId: product.id,
        monthlyPriceId: monthly.id,
        yearlyPriceId: yearly?.id ?? null,
      })
      .returning();
    return c.json(created!, 201);
  })

  .patch("/plans/:id", zValidator("json", updatePlan), async (c) => {
    const body = c.req.valid("json");
    const plan = await c.var.db.query.plans.findFirst({ where: eq(plans.id, c.req.param("id")) });
    if (!plan) return c.json({ error: "not_found" as const }, 404);

    if (body.name !== undefined || body.description !== undefined || body.active !== undefined) {
      await c.var.stripe.products.update(plan.stripeProductId, {
        name: body.name,
        description: body.description === "" ? "" : body.description,
        active: body.active,
      });
    }
    const [updated] = await c.var.db
      .update(plans)
      .set({ ...body, updatedAt: new Date() })
      .where(eq(plans.id, plan.id))
      .returning();
    return c.json(updated!);
  })

  // Prices are immutable in Stripe: create a new one, point the plan at it, archive the old.
  // Existing subscribers keep renewing at their old price.
  .post(
    "/plans/:id/prices",
    zValidator("json", z.object({ interval: z.enum(["month", "year"]), amount: amount.nullable() })),
    async (c) => {
      const { interval, amount: unitAmount } = c.req.valid("json");
      const plan = await c.var.db.query.plans.findFirst({ where: eq(plans.id, c.req.param("id")) });
      if (!plan) return c.json({ error: "not_found" as const }, 404);
      if (interval === "month" && unitAmount === null) {
        return c.json({ error: "monthly_price_required" as const }, 400);
      }

      const stripe = c.var.stripe;
      const oldPriceId = interval === "month" ? plan.monthlyPriceId : plan.yearlyPriceId;
      const price =
        unitAmount === null
          ? null
          : await stripe.prices.create({
              product: plan.stripeProductId,
              currency: plan.currency,
              unit_amount: unitAmount,
              recurring: { interval },
            });
      if (interval === "month" && price) {
        await stripe.products.update(plan.stripeProductId, { default_price: price.id });
      }
      if (oldPriceId) await stripe.prices.update(oldPriceId, { active: false });

      const [updated] = await c.var.db
        .update(plans)
        .set(
          interval === "month"
            ? { monthlyPriceId: price!.id, monthlyAmount: unitAmount, updatedAt: new Date() }
            : { yearlyPriceId: price?.id ?? null, yearlyAmount: unitAmount, updatedAt: new Date() },
        )
        .where(eq(plans.id, plan.id))
        .returning();
      return c.json(updated!);
    },
  )

  .get("/alerts", async (c) => {
    return c.json(
      await c.var.db.select().from(alerts).orderBy(asc(alerts.resolvedAt), desc(alerts.createdAt)).limit(100),
    );
  })

  .post("/alerts/:id/resolve", async (c) => {
    await c.var.db.update(alerts).set({ resolvedAt: new Date() }).where(eq(alerts.id, c.req.param("id")));
    return c.json({ ok: true });
  })

  .get("/analytics", zValidator("query", period), async (c) => {
    return c.json(await siteAnalytics(c.var.db, c.req.valid("query").days));
  })

  .get("/analytics/search", zValidator("query", period), async (c) => {
    return c.json(await searchQueries(c.env, c.req.valid("query").days));
  });
