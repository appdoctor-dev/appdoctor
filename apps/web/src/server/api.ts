import { env } from "cloudflare:workers"
import { SITE } from "#/content/site"

export type PublicPlan = {
  id: string
  name: string
  description: string
  features: string[]
  monthlyAmount: number | null
  yearlyAmount: number | null
  currency: string
  trialDays: number
  maxDevices: number
}

/** The newest GitHub release and its downloads. The asset names never change (see .github/workflows/release.yml). */
export type LatestRelease = { version: string; notes: string; mac: string; windows: string }

export const siteUrl = () => env.SITE_URL
export const appUrl = () => (env as { APP_URL?: string }).APP_URL
export const apiUrl = () => env.API_URL

/** Server-side reads from the API: a service binding in production, plain fetch locally. */
async function apiGet<T>(path: string, fallback: T): Promise<T> {
  try {
    const request = new Request(`${env.API_URL}${path}`)
    const binding = (env as { API?: Fetcher }).API
    const res = binding ? await binding.fetch(request) : await fetch(request)
    if (!res.ok) throw new Error(`${res.status}`)
    return (await res.json()) as T
  } catch (error) {
    // The marketing site should still render if the API is down; pages show their empty state.
    console.error(`[web] GET ${path} failed:`, error instanceof Error ? error.message : error)
    return fallback
  }
}

export const fetchPlans = () => apiGet<PublicPlan[]>("/v1/plans", [])

/**
 * github.com redirects /releases/latest to the newest release's tag, which
 * avoids the GitHub API's rate limit; the edge keeps the answer for five
 * minutes. `null` until the first release is published.
 */
export async function fetchLatestRelease(): Promise<LatestRelease | null> {
  try {
    const res = await fetch(`${SITE.sourceUrl}/releases/latest`, {
      redirect: "manual",
      cf: { cacheEverything: true, cacheTtlByStatus: { "200-399": 300, "400-599": 60 } },
    })
    const tag = res.headers.get("location")?.match(/\/releases\/tag\/(v[^/?#]+)$/)?.[1]
    if (!tag) return null
    const download = `${SITE.sourceUrl}/releases/download/${tag}`
    return {
      version: tag.slice(1),
      notes: `${SITE.sourceUrl}/releases/tag/${tag}`,
      mac: `${download}/AppDoctor-macOS.dmg`,
      windows: `${download}/AppDoctor-Windows-x64.zip`,
    }
  } catch (error) {
    console.error("[web] latest release lookup failed:", error instanceof Error ? error.message : error)
    return null
  }
}
