import { createFileRoute } from "@tanstack/react-router"
import { requireAccess } from "#/server/access"

export const Route = createFileRoute("/api/whoami")({
  server: {
    handlers: {
      GET: async ({ request }) => {
        const identity = await requireAccess(request)
        return identity instanceof Response ? identity : Response.json(identity)
      },
    },
  },
})
