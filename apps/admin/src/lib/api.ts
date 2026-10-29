import type { AdminApi } from "@repo/api"
import { adminClient, magicLinkClient } from "better-auth/client/plugins"
import { createAuthClient } from "better-auth/react"
import { hc } from "hono/client"

export const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:8787"

/** Typed client for the API's /admin routes; the session cookie authorizes every call. */
export const adminApi = hc<AdminApi>(`${API_URL}/admin`, { init: { credentials: "include" } })

export const authClient = createAuthClient({
  baseURL: API_URL,
  plugins: [adminClient(), magicLinkClient()],
  fetchOptions: { credentials: "include" },
})

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly body: unknown,
  ) {
    super(errorMessage(status, body))
  }
}

// Routes that return c.json(body) without a status are typed with the whole
// status union, so keep every response except ones with an explicit error status.
type ErrorStatus = 400 | 401 | 402 | 403 | 404 | 409 | 422 | 500 | 502
type OkBody<R> = R extends { status: infer S; json(): Promise<infer T> } ? ([S] extends [ErrorStatus] ? never : T) : never

/** Awaits a Hono client call and returns the success body, or throws ApiError. */
export async function call<R extends { ok: boolean; status: number; json(): Promise<unknown> }>(
  request: Promise<R>,
): Promise<OkBody<R>> {
  const res = await request
  const body = await res.json().catch(() => null)
  if (!res.ok) throw new ApiError(res.status, body)
  return body as OkBody<R>
}

function errorMessage(status: number, body: unknown) {
  if (body && typeof body === "object") {
    if ("message" in body && typeof body.message === "string") return body.message
    if ("error" in body && typeof body.error === "string") return body.error.replaceAll("_", " ")
  }
  return `Request failed (${status})`
}
