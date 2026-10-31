import { stripeClient } from "@better-auth/stripe/client"
import { deviceAuthorizationClient, magicLinkClient } from "better-auth/client/plugins"
import { createAuthClient } from "better-auth/react"

export const API_URL = import.meta.env.VITE_API_URL ?? "http://localhost:8787"

export const authClient = createAuthClient({
  // Better Auth is served at /auth on the API (not its default /api/auth).
  baseURL: `${API_URL}/auth`,
  plugins: [magicLinkClient(), deviceAuthorizationClient(), stripeClient({ subscription: true })],
  fetchOptions: { credentials: "include" },
})
