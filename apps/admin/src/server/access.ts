import { env } from "cloudflare:workers"
import { createRemoteJWKSet, jwtVerify } from "jose"

// Cloudflare Access (Zero Trust) sits in front of the admin hostname and only
// lets through people its policy allows. It also signs a JWT on every request;
// verifying it here means a request that reached this Worker some other way
// (a misconfigured route, a preview URL) is still refused.

type AccessEnv = {
  /** e.g. https://your-team.cloudflareaccess.com, set for deployed environments */
  ACCESS_TEAM_DOMAIN?: string
  /** The Access application's AUD tag */
  ACCESS_AUD?: string
  /** Local development only (.dev.vars): the identity to use when Access isn't in front. */
  ACCESS_DEV_EMAIL?: string
}

let jwks: ReturnType<typeof createRemoteJWKSet> | null = null

/** The signed-in admin's email, or a Response refusing the request. Fails closed when nothing is configured. */
export async function requireAccess(request: Request): Promise<{ email: string } | Response> {
  const { ACCESS_TEAM_DOMAIN, ACCESS_AUD, ACCESS_DEV_EMAIL } = env as AccessEnv

  if (ACCESS_TEAM_DOMAIN && ACCESS_AUD) {
    const token = request.headers.get("cf-access-jwt-assertion")
    if (!token) return forbidden("Missing Cloudflare Access token")
    jwks ??= createRemoteJWKSet(new URL(`${ACCESS_TEAM_DOMAIN}/cdn-cgi/access/certs`))
    try {
      const { payload } = await jwtVerify(token, jwks, { issuer: ACCESS_TEAM_DOMAIN, audience: ACCESS_AUD })
      if (typeof payload.email !== "string") return forbidden("Access token has no email")
      return { email: payload.email }
    } catch {
      return forbidden("Invalid Cloudflare Access token")
    }
  }

  if (ACCESS_DEV_EMAIL) return { email: ACCESS_DEV_EMAIL }
  return forbidden("Cloudflare Access is not configured for this environment")
}

function forbidden(message: string) {
  return Response.json({ error: "forbidden", message }, { status: 403 })
}
