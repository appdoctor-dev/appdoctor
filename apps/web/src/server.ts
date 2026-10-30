/**
 * Worker entry (wrangler `main`). Wraps TanStack Start's handler to serve the
 * agent endpoints and Markdown versions of pages (src/server/agent.ts), and to
 * add discovery Link headers to every HTML page.
 *
 * One Worker answers three hosts: the website (appdoctor.so), www (which
 * redirects to it), and app.appdoctor.so, where people sign in and manage
 * their account. Each page lives on one of them; the other redirects.
 */
import handler, { createServerEntry } from "@tanstack/react-start/server-entry"
import { appUrl, siteUrl } from "./server/api"
import { decorateHtml, handleAgentRequest } from "./server/agent"

/** Pages that belong to the app host. */
const APP_PATHS = ["/login", "/account", "/device"]
const isAppPath = (path: string) => APP_PATHS.some((p) => path === p || path.startsWith(`${p}/`))
/** Framework requests both hosts must answer (server functions, assets). */
const isShared = (path: string) => path.startsWith("/_serverFn") || path.startsWith("/assets/") || path.includes(".")

function moved(to: string, url: URL) {
  return Response.redirect(`${to}${url.pathname}${url.search}`, 301)
}

export default createServerEntry({
  async fetch(request, opts) {
    const url = new URL(request.url)
    // One address for the site: www goes to the apex, path and all.
    if (url.hostname.startsWith("www.")) {
      url.hostname = url.hostname.slice(4)
      return Response.redirect(url.toString(), 301)
    }

    const site = siteUrl() ?? url.origin
    const app = appUrl()
    const separateApp = app && new URL(app).host !== new URL(site).host
    if (separateApp && url.host === new URL(app).host) {
      // The app host: sign-in and the account, kept out of search.
      if (url.pathname === "/robots.txt") return new Response("User-agent: *\nDisallow: /\n", { headers: { "content-type": "text/plain" } })
      if (url.pathname === "/") return Response.redirect(`${app}/account`, 302)
      if (!isAppPath(url.pathname) && !isShared(url.pathname)) return moved(site, url)
      const response = await handler.fetch(request, opts)
      const headers = new Headers(response.headers)
      headers.set("X-Robots-Tag", "noindex")
      return new Response(response.body, { status: response.status, statusText: response.statusText, headers })
    }
    if (separateApp && isAppPath(url.pathname)) return moved(app, url)

    if (request.method === "GET" || request.method === "HEAD") {
      const agentResponse = await handleAgentRequest(request, site)
      if (agentResponse) return agentResponse
    }
    return decorateHtml(await handler.fetch(request, opts), url)
  },
})
