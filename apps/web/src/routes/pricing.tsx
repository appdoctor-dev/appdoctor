import { createFileRoute, useNavigate } from "@tanstack/react-router"
import { useState } from "react"
import { PageIntro, buttonClass } from "#/components/chrome"
import { SITE } from "#/content/site"
import { authClient } from "#/lib/auth"
import { getPlans } from "#/lib/data.functions"
import { money } from "#/lib/format"

type Search = { plan?: string; annual?: boolean }

export const Route = createFileRoute("/pricing")({
  validateSearch: (search: Record<string, unknown>): Search => ({
    plan: typeof search.plan === "string" ? search.plan : undefined,
    annual: search.annual === true || search.annual === "true" ? true : undefined,
  }),
  loader: () => getPlans(),
  head: () => ({
    meta: [
      { title: `Pricing: ${SITE.name} ASO tool` },
      { name: "description", content: `${SITE.name} pricing. One subscription for the signed Mac and Windows apps, updates and support.` },
    ],
  }),
  component: Pricing,
})

function Pricing() {
  const plans = Route.useLoaderData()
  const search = Route.useSearch()
  const [annual, setAnnual] = useState(search.annual ?? false)
  const hasYearly = plans.some((p) => p.yearlyAmount)

  return (
    <>
      <PageIntro title="One subscription, every feature">
        The source code is free to read and build. The subscription gets you signed apps for Mac and Windows, automatic
        updates and support. Cancel any time.
      </PageIntro>
      <div className="mx-auto max-w-6xl px-4 pb-20 sm:px-6">
        {hasYearly && (
          <div className="mb-8 inline-flex rounded-lg border border-line bg-surface p-1 text-sm" role="group" aria-label="Billing period">
            {[false, true].map((yearly) => (
              <button
                key={String(yearly)}
                type="button"
                aria-pressed={annual === yearly}
                onClick={() => setAnnual(yearly)}
                className={`rounded-md px-4 py-1.5 font-medium ${annual === yearly ? "bg-ink text-paper" : "text-ink-soft"}`}
              >
                {yearly ? "Yearly" : "Monthly"}
              </button>
            ))}
          </div>
        )}
        {plans.length === 0 ? (
          <p className="rounded-2xl border border-line bg-surface p-8 text-ink-soft">
            Plans are being finalized. Check back soon, or{" "}
            <a className="underline underline-offset-4" href={`mailto:${SITE.email}`}>
              email us
            </a>
            .
          </p>
        ) : (
          <div className="grid gap-6 md:grid-cols-2 lg:grid-cols-3">
            {plans.map((plan) => (
              <PlanCard key={plan.id} plan={plan} annual={annual && !!plan.yearlyAmount} highlighted={search.plan === plan.id} />
            ))}
          </div>
        )}
      </div>
    </>
  )
}

function PlanCard({
  plan,
  annual,
  highlighted,
}: {
  plan: Awaited<ReturnType<typeof getPlans>>[number]
  annual: boolean
  highlighted: boolean
}) {
  const navigate = useNavigate()
  const { data: session } = authClient.useSession()
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  async function subscribe() {
    if (!session) {
      navigate({ to: "/login", search: { redirect: `/pricing?plan=${plan.id}${annual ? "&annual=true" : ""}` } })
      return
    }
    setBusy(true)
    setError(null)
    const origin = window.location.origin
    // Redirects to Stripe Checkout; Stripe sends the customer back to their account page.
    const { error } = await authClient.subscription.upgrade({
      plan: plan.id,
      annual,
      successUrl: `${origin}/account?welcome=1`,
      cancelUrl: `${origin}/pricing`,
    })
    if (error) {
      setError(error.message ?? "Checkout couldn't start. Try again.")
      setBusy(false)
    }
  }

  const amount = annual ? plan.yearlyAmount : plan.monthlyAmount
  return (
    <div className={`flex flex-col rounded-2xl border bg-surface p-6 ${highlighted ? "border-ink" : "border-line"}`}>
      <h2 className="text-xl font-semibold">{plan.name}</h2>
      {plan.description && <p className="mt-1 text-ink-soft">{plan.description}</p>}
      <p className="mt-6">
        <span className="text-4xl font-bold tabular-nums">{money(amount, plan.currency)}</span>
        <span className="text-ink-soft"> / {annual ? "year" : "month"}</span>
      </p>
      <ul className="mt-6 flex flex-1 flex-col gap-2 text-sm">
        <li>
          Use it on {plan.maxDevices} computer{plan.maxDevices === 1 ? "" : "s"}
        </li>
        {plan.features.map((f) => (
          <li key={f}>{f}</li>
        ))}
      </ul>
      <button type="button" onClick={subscribe} disabled={busy} className={`${buttonClass("primary")} mt-8 w-full`}>
        {busy ? "Opening checkout…" : plan.trialDays > 0 ? `Start ${plan.trialDays}-day free trial` : `Subscribe to ${plan.name}`}
      </button>
      {error && <p className="mt-3 text-sm text-down">{error}</p>}
    </div>
  )
}
