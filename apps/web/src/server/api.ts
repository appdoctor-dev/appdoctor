import { env } from "cloudflare:workers"

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

export type LatestBuild = { platform: "macos" | "windows"; arch: "arm64" | "x64"; version: string; size: number; notes: string }

export const siteUrl = () => env.SITE_URL
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
export const fetchLatestBuilds = () => apiGet<LatestBuild[]>("/v1/releases/latest", [])
