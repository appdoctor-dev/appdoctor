import { Navigate, createFileRoute } from "@tanstack/react-router"
import { useState } from "react"
import { buttonClass } from "#/components/chrome"
import { authClient } from "#/lib/auth"

// The desktop app opens this page during sign-in, with the code it is showing.

type Search = { user_code?: string }

export const Route = createFileRoute("/device")({
  ssr: false,
  validateSearch: (search: Record<string, unknown>): Search => ({
    user_code: typeof search.user_code === "string" ? search.user_code : undefined,
  }),
  head: () => ({ meta: [{ title: "Sign in the AppDoctor app" }, { name: "robots", content: "noindex" }] }),
  component: Device,
})

function Device() {
  const { user_code } = Route.useSearch()
  const { data: session, isPending } = authClient.useSession()
  const [code, setCode] = useState(user_code ?? "")
  const [state, setState] = useState<"idle" | "working" | "approved" | "denied">("idle")
  const [error, setError] = useState<string | null>(null)

  if (isPending) return null
  if (!session) {
    const back = `/device${user_code ? `?user_code=${encodeURIComponent(user_code)}` : ""}`
    return <Navigate to="/login" search={{ redirect: back }} />
  }

  async function decide(approve: boolean) {
    setState("working")
    setError(null)
    const userCode = code.trim().toUpperCase()
    const { error } = approve
      ? await authClient.device.approve({ userCode })
      : await authClient.device.deny({ userCode })
    if (error) {
      setError(
        error.error === "expired_token"
          ? "That code has expired. Choose Sign in again in the app to get a new one."
          : error.error_description || "That code didn't work. Check it matches the one in the app.",
      )
      setState("idle")
    } else {
      setState(approve ? "approved" : "denied")
    }
  }

  return (
    <div className="mx-auto max-w-md px-4 py-20">
      {state === "approved" ? (
        <>
          <h1 className="text-3xl font-bold tracking-tight">You're signed in</h1>
          <p className="mt-3 text-ink-soft">Go back to the AppDoctor app. You can close this tab.</p>
        </>
      ) : state === "denied" ? (
        <>
          <h1 className="text-3xl font-bold tracking-tight">Sign-in canceled</h1>
          <p className="mt-3 text-ink-soft">The app was not signed in. You can close this tab.</p>
        </>
      ) : (
        <>
          <h1 className="text-3xl font-bold tracking-tight">Sign in the AppDoctor app</h1>
          <p className="mt-3 text-ink-soft">
            Check that this code matches the one in the app, then approve. You're signed in as {session.user.email}.
          </p>
          <label htmlFor="code" className="mt-8 block text-sm font-medium">
            Code from the app
          </label>
          <input
            id="code"
            value={code}
            onChange={(e) => setCode(e.target.value)}
            autoComplete="off"
            className="mt-2 h-14 w-full rounded-lg border border-line bg-surface px-3 text-center text-2xl font-semibold tracking-[0.3em] uppercase"
          />
          <div className="mt-6 flex gap-3">
            <button type="button" disabled={!code || state === "working"} onClick={() => decide(true)} className={`${buttonClass("primary")} flex-1`}>
              Approve
            </button>
            <button type="button" disabled={!code || state === "working"} onClick={() => decide(false)} className={buttonClass("secondary")}>
              Deny
            </button>
          </div>
          {error && <p className="mt-4 text-sm text-down">{error}</p>}
        </>
      )}
    </div>
  )
}
