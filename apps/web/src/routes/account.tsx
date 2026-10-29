import { Link, Navigate, createFileRoute } from "@tanstack/react-router"
import { useCallback, useEffect, useState } from "react"
import { buttonClass } from "#/components/chrome"
import { API_URL, authClient } from "#/lib/auth"
import { date } from "#/lib/format"

type Me = {
  user: { email: string; name: string }
  subscription: {
    planName: string
    status: string | null
    interval: string | null
    periodEnd: string | null
    cancelAtPeriodEnd: boolean | null
    trialEnd: string | null
    maxDevices: number
  } | null
  devices: { deviceId: string; deviceName: string; platform: string; appVersion: string; lastSeenAt: string }[]
}

export const Route = createFileRoute("/account")({
  ssr: false,
  validateSearch: (search: Record<string, unknown>): { welcome?: boolean } => ({
    welcome: search.welcome === 1 || search.welcome === "1" ? true : undefined,
  }),
  head: () => ({ meta: [{ title: "Your account: AppDoctor" }, { name: "robots", content: "noindex" }] }),
  component: Account,
})

function Account() {
  const { welcome } = Route.useSearch()
  const { data: session, isPending } = authClient.useSession()
  const [me, setMe] = useState<Me | null>(null)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    const res = await fetch(`${API_URL}/v1/me`, { credentials: "include" })
    if (res.ok) setMe(await res.json())
    else setError("Your account couldn't be loaded. Refresh to try again.")
  }, [])

  useEffect(() => {
    if (session) void load()
  }, [session, load])

  if (isPending) return null
  if (!session) return <Navigate to="/login" search={{ redirect: "/account" }} />

  async function manageBilling() {
    const { error } = await authClient.subscription.billingPortal({ returnUrl: `${window.location.origin}/account` })
    if (error) setError(error.message ?? "Billing couldn't be opened. Try again.")
  }

  async function signOutDevice(deviceId: string) {
    await fetch(`${API_URL}/v1/devices/${encodeURIComponent(deviceId)}`, { method: "DELETE", credentials: "include" })
    await load()
  }

  const sub = me?.subscription
  return (
    <div className="mx-auto max-w-3xl px-4 py-16 sm:px-6">
      <h1 className="text-4xl font-bold tracking-tight">Your account</h1>
      <p className="mt-2 text-ink-soft">{session.user.email}</p>
      {welcome && (
        <p className="mt-6 rounded-xl border border-up/30 bg-tint p-4">
          Thanks for subscribing. <Link to="/download" className="font-semibold underline underline-offset-4">Download AppDoctor</Link>,
          open it and choose Sign in.
        </p>
      )}
      {error && <p className="mt-6 text-down">{error}</p>}

      <section className="mt-10 rounded-2xl border border-line bg-surface p-6">
        <h2 className="text-lg font-semibold">Subscription</h2>
        {!me ? (
          <p className="mt-2 text-ink-soft">Loading…</p>
        ) : sub ? (
          <>
            <p className="mt-2">
              <strong>{sub.planName}</strong>, {sub.interval === "year" ? "billed yearly" : "billed monthly"}.{" "}
              {sub.status === "trialing" && sub.trialEnd
                ? `Free trial until ${date(sub.trialEnd)}.`
                : sub.cancelAtPeriodEnd
                  ? `Ends on ${date(sub.periodEnd)}.`
                  : sub.status === "past_due"
                    ? "Your last payment failed. Update your card to keep using the app."
                    : `Renews on ${date(sub.periodEnd)}.`}
            </p>
            <button type="button" onClick={manageBilling} className={`${buttonClass("secondary", "sm")} mt-4`}>
              Manage billing
            </button>
          </>
        ) : (
          <>
            <p className="mt-2 text-ink-soft">You don't have a subscription.</p>
            <Link to="/pricing" className={`${buttonClass("primary", "sm")} mt-4`}>
              See plans
            </Link>
          </>
        )}
      </section>

      {me && (
        <section className="mt-6 rounded-2xl border border-line bg-surface p-6">
          <h2 className="text-lg font-semibold">Computers</h2>
          <p className="mt-1 text-sm text-ink-soft">
            {sub ? `You can use AppDoctor on ${sub.maxDevices} computers. ` : ""}Sign one out to make room for another.
          </p>
          {me.devices.length === 0 ? (
            <p className="mt-4 text-ink-soft">
              Not signed in anywhere yet. <Link to="/download" className="underline underline-offset-4">Download the app</Link>.
            </p>
          ) : (
            <ul className="mt-4 divide-y divide-line">
              {me.devices.map((d) => (
                <li key={d.deviceId} className="flex items-center justify-between gap-4 py-3">
                  <span>
                    <span className="font-medium">{d.deviceName || "Unnamed computer"}</span>
                    <span className="block text-sm text-ink-soft">
                      {d.platform === "macos" ? "Mac" : "Windows"} · last used {date(d.lastSeenAt)}
                    </span>
                  </span>
                  <button type="button" onClick={() => signOutDevice(d.deviceId)} className="text-sm underline underline-offset-4">
                    Sign out
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}

      <button type="button" onClick={() => authClient.signOut().then(() => (window.location.href = "/"))} className="mt-10 text-sm text-ink-soft underline underline-offset-4">
        Sign out of this website
      </button>
    </div>
  )
}
