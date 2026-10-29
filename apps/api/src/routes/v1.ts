import { zValidator } from "@hono/zod-validator";
import { activations, plans } from "@repo/db";
import { and, asc, eq } from "drizzle-orm";
import { Hono } from "hono";
import { z } from "zod";
import type { AppEnv } from "../env";
import { currentSubscription, entitlementExpiry, signEntitlement } from "../lib/entitlement";
import { ARCHES, compareVersions, latestRelease, PLATFORMS } from "../lib/releases";
import { requireUser } from "../middleware";

const target = z.object({ platform: z.enum(PLATFORMS), arch: z.enum(ARCHES) });

const device = z.object({
  // A random UUID the app creates on first launch; never a hardware ID.
  deviceId: z.uuid(),
  deviceName: z.string().max(100).default(""),
  platform: z.enum(["macos", "windows", "linux"]),
  appVersion: z.string().max(32).default(""),
});

/** Public and signed-in-customer endpoints, used by the website and the desktop app. */
export const v1 = new Hono<AppEnv>()
  .get("/plans", async (c) => {
    const rows = await c.var.db
      .select()
      .from(plans)
      .where(eq(plans.active, true))
      .orderBy(asc(plans.sort));
    return c.json(
      rows
        .filter((p) => p.monthlyPriceId)
        .map((p) => ({
          id: p.id,
          name: p.name,
          description: p.description,
          features: p.features,
          monthlyAmount: p.monthlyAmount,
          yearlyAmount: p.yearlyPriceId ? p.yearlyAmount : null,
          currency: p.currency,
          trialDays: p.trialDays,
          maxDevices: p.maxDevices,
        })),
    );
  })

  .get("/me", requireUser, async (c) => {
    const user = c.var.user!;
    const graceDays = Number(c.env.LICENSE_GRACE_DAYS ?? 7);
    const sub = await currentSubscription(c.var.db, user.id, graceDays);
    const plan = sub ? await c.var.db.query.plans.findFirst({ where: eq(plans.id, sub.plan) }) : null;
    const devices = await c.var.db.select().from(activations).where(eq(activations.userId, user.id));
    return c.json({
      user: { id: user.id, email: user.email, name: user.name, role: user.role },
      subscription: sub && {
        plan: sub.plan,
        planName: plan?.name ?? sub.plan,
        status: sub.status,
        interval: sub.billingInterval,
        periodEnd: sub.periodEnd,
        cancelAtPeriodEnd: sub.cancelAtPeriodEnd,
        trialEnd: sub.trialEnd,
        maxDevices: plan?.maxDevices ?? 0,
      },
      devices: devices.map((d) => ({
        deviceId: d.deviceId,
        deviceName: d.deviceName,
        platform: d.platform,
        appVersion: d.appVersion,
        lastSeenAt: d.lastSeenAt,
      })),
    });
  })

  // The desktop app calls this on sign-in and about daily; the token works offline until it expires.
  .post("/entitlement", requireUser, zValidator("json", device), async (c) => {
    const user = c.var.user!;
    const body = c.req.valid("json");
    const graceDays = Number(c.env.LICENSE_GRACE_DAYS ?? 7);

    const sub = await currentSubscription(c.var.db, user.id, graceDays);
    if (!sub) return c.json({ error: "no_subscription" as const }, 402);
    const plan = await c.var.db.query.plans.findFirst({ where: eq(plans.id, sub.plan) });
    const maxDevices = plan?.maxDevices ?? 1;

    const devices = await c.var.db.select().from(activations).where(eq(activations.userId, user.id));
    const known = devices.some((d) => d.deviceId === body.deviceId);
    if (!known && devices.length >= maxDevices) {
      return c.json(
        {
          error: "device_limit" as const,
          maxDevices,
          devices: devices.map((d) => ({ deviceId: d.deviceId, deviceName: d.deviceName, lastSeenAt: d.lastSeenAt })),
        },
        409,
      );
    }

    await c.var.db
      .insert(activations)
      .values({ id: crypto.randomUUID(), userId: user.id, ...body })
      .onConflictDoUpdate({
        target: [activations.userId, activations.deviceId],
        set: { deviceName: body.deviceName, platform: body.platform, appVersion: body.appVersion, lastSeenAt: new Date() },
      });

    const now = Date.now();
    const expiresAt = entitlementExpiry(sub.periodEnd, graceDays, now);
    const token = await signEntitlement(c.env.LICENSE_SIGNING_KEY, {
      v: 1,
      sub: user.id,
      plan: sub.plan,
      device: body.deviceId,
      iat: Math.floor(now / 1000),
      exp: Math.floor(expiresAt / 1000),
    });
    return c.json({ token, expiresAt: new Date(expiresAt).toISOString(), plan: sub.plan, status: sub.status });
  })

  .delete("/devices/:deviceId", requireUser, async (c) => {
    await c.var.db
      .delete(activations)
      .where(and(eq(activations.userId, c.var.user!.id), eq(activations.deviceId, c.req.param("deviceId"))));
    return c.json({ ok: true });
  })

  .get("/releases/latest", async (c) => {
    const builds = await Promise.all(
      (["macos", "windows"] as const).flatMap((platform) =>
        (["arm64", "x64"] as const).map(async (arch) => {
          const release = await latestRelease(c.var.db, platform, arch);
          return release && { platform, arch, version: release.version, size: release.size, notes: release.notes };
        }),
      ),
    );
    return c.json(builds.filter((b) => b !== null));
  })

  .get("/downloads/:platform/:arch", zValidator("param", target), async (c) => {
    const { platform, arch } = c.req.valid("param");
    const release = await latestRelease(c.var.db, platform, arch);
    const object = release && (await c.env.RELEASES.get(release.r2Key));
    if (!release || !object) return c.json({ error: "not_found" as const }, 404);
    return new Response(object.body, {
      headers: {
        "content-type": object.httpMetadata?.contentType ?? "application/octet-stream",
        "content-length": String(object.size),
        "content-disposition": `attachment; filename="${release.filename}"`,
        etag: object.httpEtag,
      },
    });
  })

  // Desktop auto-update check: 204 when current, otherwise the newer build's manifest.
  .get(
    "/updates/:platform/:arch/:version",
    zValidator("param", target.extend({ version: z.string().max(32) })),
    async (c) => {
      const { platform, arch, version } = c.req.valid("param");
      const release = await latestRelease(c.var.db, platform, arch);
      if (!release || compareVersions(release.version, version) <= 0) return c.body(null, 204);
      return c.json({
        version: release.version,
        notes: release.notes,
        pubDate: release.publishedAt,
        url: `${c.env.API_URL}/v1/downloads/${platform}/${arch}`,
        signature: release.signature,
        size: release.size,
      });
    },
  );
