import { alerts, type Db } from "@repo/db";
import type Stripe from "stripe";

// The Better Auth Stripe plugin keeps subscriptions in sync. These are the
// money-at-risk events it does not handle: each becomes an admin alert so a
// person decides whether to refund, cancel or fight it.

export async function handleRiskEvent(stripe: Stripe, db: Db, event: Stripe.Event) {
  switch (event.type) {
    case "charge.refunded": {
      const charge = event.data.object;
      const owner = await ownerOfPayment(stripe, charge.payment_intent);
      await recordAlert(db, event, {
        kind: "refund",
        stripeObjectId: charge.id,
        stripeCustomerId: idOf(charge.customer) ?? owner.customerId,
        stripeSubscriptionId: owner.subscriptionId,
        amount: charge.amount_refunded,
        currency: charge.currency,
        message: charge.refunded ? "Charge fully refunded" : "Charge partially refunded",
      });
      return;
    }
    case "charge.dispute.created": {
      const dispute = event.data.object;
      const owner = await ownerOfPayment(stripe, dispute.payment_intent);
      await recordAlert(db, event, {
        kind: "dispute",
        stripeObjectId: dispute.id,
        stripeCustomerId: owner.customerId,
        stripeSubscriptionId: owner.subscriptionId,
        amount: dispute.amount,
        currency: dispute.currency,
        message: `Dispute opened (${dispute.reason}). Respond in the Stripe Dashboard before the deadline.`,
      });
      return;
    }
    case "radar.early_fraud_warning.created": {
      const warning = event.data.object;
      let paymentIntent: string | Stripe.PaymentIntent | null | undefined = warning.payment_intent;
      if (!paymentIntent) {
        const charge = await stripe.charges.retrieve(idOf(warning.charge)!);
        paymentIntent = charge.payment_intent;
      }
      const owner = await ownerOfPayment(stripe, paymentIntent);
      await recordAlert(db, event, {
        kind: "fraud_warning",
        stripeObjectId: warning.id,
        stripeCustomerId: owner.customerId,
        stripeSubscriptionId: owner.subscriptionId,
        message: `Early fraud warning (${warning.fraud_type}). Consider refunding and canceling to avoid a dispute.`,
      });
      return;
    }
  }
}

async function recordAlert(
  db: Db,
  event: Stripe.Event,
  alert: Omit<typeof alerts.$inferInsert, "id">,
) {
  // Keyed by event ID, so Stripe's retries do not duplicate alerts.
  await db
    .insert(alerts)
    .values({ id: event.id, ...alert })
    .onConflictDoNothing();
}

/** Payment → invoice → subscription, through Stripe's object graph rather than metadata. */
async function ownerOfPayment(
  stripe: Stripe,
  paymentIntent: string | Stripe.PaymentIntent | null | undefined,
): Promise<{ customerId: string | null; subscriptionId: string | null }> {
  const paymentIntentId = idOf(paymentIntent);
  if (!paymentIntentId) return { customerId: null, subscriptionId: null };

  const payments = await stripe.invoicePayments.list({
    payment: { type: "payment_intent", payment_intent: paymentIntentId },
    limit: 1,
    expand: ["data.invoice"],
  });
  const invoiceRef = payments.data[0]?.invoice;
  if (!invoiceRef) return { customerId: null, subscriptionId: null };

  const invoice = typeof invoiceRef === "string" ? await stripe.invoices.retrieve(invoiceRef) : invoiceRef;
  if (!("customer" in invoice)) return { customerId: null, subscriptionId: null };
  return {
    customerId: idOf(invoice.customer),
    subscriptionId: idOf(invoice.parent?.subscription_details?.subscription ?? null),
  };
}

function idOf(ref: string | { id: string } | null | undefined): string | null {
  if (!ref) return null;
  return typeof ref === "string" ? ref : ref.id;
}
