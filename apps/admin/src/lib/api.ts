import type { AdminApi } from "@repo/api"
import { hc } from "hono/client"

/** Typed client for the API's admin routes, through this app's Access-protected proxy (routes/api/admin/$.ts). */
export const adminApi = hc<AdminApi>("/api/admin")

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
