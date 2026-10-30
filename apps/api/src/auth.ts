import { stripe } from "@better-auth/stripe";
import { type Db, plans, schema } from "@repo/db";
import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { bearer, deviceAuthorization, magicLink } from "better-auth/plugins";
import { asc, eq } from "drizzle-orm";
import Stripe from "stripe";
import type { Bindings } from "./env";
import { sendEmail } from "./lib/email";
import { handleRiskEvent } from "./lib/stripe-events";

/** The OAuth client ID the desktop app uses for the device sign-in flow. */
export const DESKTOP_CLIENT_ID = "appdoctor-desktop";

// Tags Checkout Sessions in the Stripe Dashboard so this flow can be compared with others.
const CHECKOUT_INTEGRATION_ID = "appdoctor_subscribe_kwtqzmrb";

export function createStripe(env: Bindings) {
  return new Stripe(env.STRIPE_SECRET_KEY);
}

/** Active plans in the shape the Better Auth Stripe plugin expects; the admin edits these. */
export async function loadStripePlans(db: Db) {
  const rows = await db.select().from(plans).where(eq(plans.active, true)).orderBy(asc(plans.sort));
  return rows
    .filter((p) => p.monthlyPriceId)
    .map((p) => ({
      name: p.id,
      priceId: p.monthlyPriceId ?? undefined,
      annualDiscountPriceId: p.yearlyPriceId ?? undefined,
      freeTrial: p.trialDays > 0 ? { days: p.trialDays } : undefined,
      limits: { devices: p.maxDevices },
    }));
}

export function createAuth(env: Bindings, db: Db, stripeClient: Stripe = createStripe(env)) {
  return betterAuth({
    appName: "AppDoctor",
    baseURL: env.API_URL,
    secret: env.BETTER_AUTH_SECRET,
    trustedOrigins: [env.WEB_URL, ...(env.APP_URL ? [env.APP_URL] : [])],
    database: drizzleAdapter(db, { provider: "sqlite", schema }),
    advanced: env.COOKIE_DOMAIN
      ? { crossSubDomainCookies: { enabled: true, domain: env.COOKIE_DOMAIN } }
      : undefined,
    socialProviders:
      env.GOOGLE_CLIENT_ID && env.GOOGLE_CLIENT_SECRET
        ? { google: { clientId: env.GOOGLE_CLIENT_ID, clientSecret: env.GOOGLE_CLIENT_SECRET } }
        : undefined,
    plugins: [
      // Passwordless: people buying a desktop app should not have to invent a password.
      magicLink({
        sendMagicLink: async ({ email, url }) => {
          await sendEmail(env, {
            to: email,
            subject: "Your AppDoctor sign-in link",
            text: `Sign in to AppDoctor:\n\n${url}\n\nThis link expires in 5 minutes. If you didn't ask for it, ignore this email.`,
          });
        },
      }),
      // Lets the desktop app call the API with the token from the device flow.
      bearer(),
      deviceAuthorization({
        verificationUri: `${env.APP_URL ?? env.WEB_URL}/device`,
        validateClient: (clientId) => clientId === DESKTOP_CLIENT_ID,
      }),
      stripe({
        stripeClient,
        stripeWebhookSecret: env.STRIPE_WEBHOOK_SECRET,
        // Only buyers reach Stripe; signing in to look around creates no Stripe customer.
        createCustomerOnSignUp: false,
        subscription: {
          enabled: true,
          plans: () => loadStripePlans(db),
          getCheckoutSessionParams: () => ({
            params: {
              allow_promotion_codes: true,
              // Collects nothing until you add tax registrations in Stripe; see README.
              automatic_tax: { enabled: env.STRIPE_AUTOMATIC_TAX === "true" },
              integration_identifier: CHECKOUT_INTEGRATION_ID,
            },
          }),
        },
        onEvent: (event) => handleRiskEvent(stripeClient, db, event),
      }),
    ],
  });
}

export type Auth = ReturnType<typeof createAuth>;
