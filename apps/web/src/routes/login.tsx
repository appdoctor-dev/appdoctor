import { createFileRoute } from "@tanstack/react-router"
import { useEffect, useRef, useState } from "react"
import { buttonClass } from "#/components/chrome"
import { authClient } from "#/lib/auth"

type Search = { redirect?: string; token?: string }

export const Route = createFileRoute("/login")({
  ssr: false,
  validateSearch: (search: Record<string, unknown>): Search => ({
    // Only same-site paths, so the link can't bounce someone to another site after sign-in.
    redirect: typeof search.redirect === "string" && search.redirect.startsWith("/") && !search.redirect.startsWith("//") ? search.redirect : undefined,
    // From the link in the sign-in email.
    token: typeof search.token === "string" ? search.token : undefined,
  }),
  head: () => ({ meta: [{ title: "Sign in: AppDoctor" }, { name: "robots", content: "noindex" }] }),
  component: Login,
})

function Login() {
  const { redirect = "/account", token } = Route.useSearch()
  const [email, setEmail] = useState("")
  const [state, setState] = useState<"idle" | "sending" | "sent" | "verifying">(token ? "verifying" : "idle")
  const [error, setError] = useState<string | null>(null)
  const callbackURL = `${window.location.origin}${redirect}`

  // The emailed link lands here with its token: redeem it once (it only
  // works once), then go on, replacing this URL so Back doesn't reuse it.
  const redeemed = useRef(false)
  useEffect(() => {
    if (!token || redeemed.current) return
    redeemed.current = true
    // A good token answers with the new session; a bad one answers with a
    // redirect to an error page instead of an error, so check for the session.
    authClient.magicLink.verify({ query: { token } }).then(({ data }) => {
      if (data && typeof data === "object" && "session" in data) {
        window.location.replace(redirect)
      } else {
        setError("That sign-in link has expired or was already used. Send yourself a new one.")
        setState("idle")
      }
    })
  }, [token, redirect])

  async function sendLink(e: React.FormEvent) {
    e.preventDefault()
    setState("sending")
    setError(null)
    const { error } = await authClient.signIn.magicLink({ email, callbackURL })
    if (error) {
      setError(error.message ?? "The sign-in link couldn't be sent. Check the address and try again.")
      setState("idle")
    } else {
      setState("sent")
    }
  }

  async function google() {
    const { error } = await authClient.signIn.social({ provider: "google", callbackURL })
    if (error) setError(error.message ?? "Google sign-in isn't available right now. Use your email instead.")
  }

  return (
    <div className="mx-auto max-w-md px-4 py-20">
      <h1 className="text-3xl font-bold tracking-tight">Sign in to AppDoctor</h1>
      {state === "verifying" ? (
        <p className="mt-3 text-ink-soft" aria-live="polite">
          Signing you in…
        </p>
      ) : state === "sent" ? (
        <div className="mt-6 rounded-2xl border border-line bg-surface p-6">
          <p className="font-semibold">Check your email</p>
          <p className="mt-2 text-ink-soft">
            We sent a sign-in link to <strong className="text-ink">{email}</strong>. It works for 5 minutes.
          </p>
          <button type="button" className="mt-4 text-sm underline underline-offset-4" onClick={() => setState("idle")}>
            Use a different email
          </button>
        </div>
      ) : (
        <>
          <p className="mt-3 text-ink-soft">No password needed. We'll email you a link that signs you in.</p>
          <form onSubmit={sendLink} className="mt-8 flex flex-col gap-3">
            <label htmlFor="email" className="text-sm font-medium">
              Email
            </label>
            <input
              id="email"
              type="email"
              required
              autoComplete="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              className="h-11 rounded-lg border border-line bg-surface px-3"
            />
            <button type="submit" disabled={state === "sending"} className={buttonClass("primary")}>
              {state === "sending" ? "Sending…" : "Email me a sign-in link"}
            </button>
          </form>
          <div className="my-6 flex items-center gap-3 text-xs text-ink-soft">
            <span className="h-px flex-1 bg-line" />
            or
            <span className="h-px flex-1 bg-line" />
          </div>
          <button type="button" onClick={google} className={`${buttonClass("secondary")} w-full`}>
            Continue with Google
          </button>
          {error && <p className="mt-4 text-sm text-down">{error}</p>}
        </>
      )}
    </div>
  )
}
