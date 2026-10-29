import { createFileRoute } from "@tanstack/react-router"
import { env } from "cloudflare:workers"
import { requireAccess } from "#/server/access"

// Same-origin proxy from the admin UI to the API's admin routes. The API only
// serves those routes through the private `API` service binding, never on the
// public internet, and trusts the identity this Worker verified.

const FORWARDED_HEADERS = ["content-type", "content-length", "accept"]

export const Route = createFileRoute("/api/admin/$")({
  server: {
    handlers: {
      ANY: async ({ request, params }) => {
        const identity = await requireAccess(request)
        if (identity instanceof Response) return identity

        const url = new URL(request.url)
        const headers = new Headers()
        for (const name of FORWARDED_HEADERS) {
          const value = request.headers.get(name)
          if (value) headers.set(name, value)
        }
        headers.set("x-admin-email", identity.email)

        const hasBody = request.method !== "GET" && request.method !== "HEAD"
        return (env as { API: Fetcher }).API.fetch(
          new Request(`https://api.internal/admin/${params._splat ?? ""}${url.search}`, {
            method: request.method,
            headers,
            body: hasBody ? request.body : undefined,
            // @ts-expect-error duplex is required by fetch for streamed bodies but missing from the DOM types
            duplex: hasBody ? "half" : undefined,
          }),
        )
      },
    },
  },
})
