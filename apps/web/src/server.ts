/**
 * Worker entry (wrangler `main`). Wraps TanStack Start's handler to serve the
 * agent endpoints and Markdown versions of pages (src/server/agent.ts), and to
 * add discovery Link headers to every HTML page.
 */
import handler, { createServerEntry } from "@tanstack/react-start/server-entry"
import { siteUrl } from "./server/api"
import { decorateHtml, handleAgentRequest } from "./server/agent"

export default createServerEntry({
  async fetch(request, opts) {
    const url = new URL(request.url)
    if (request.method === "GET" || request.method === "HEAD") {
      const agentResponse = await handleAgentRequest(request, siteUrl() ?? url.origin)
      if (agentResponse) return agentResponse
    }
    return decorateHtml(await handler.fetch(request, opts), url)
  },
})
