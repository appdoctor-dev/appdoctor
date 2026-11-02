/**
 * Site analytics without cookies or IP addresses. For each person arriving
 * from outside the site it records the page, the country Cloudflare places
 * them in, where they came from and any search terms that came along; for
 * each AI or search agent reading a page, which agent it was. The API keeps
 * the rows (SiteEntrypoint) and the admin's Analytics page reads them.
 */
import { env, waitUntil } from "cloudflare:workers"

type Channel = "search" | "ai" | "social" | "referral" | "email" | "campaign" | "direct"
type AgentKind = "assistant" | "ai-search" | "ai-training" | "search"

type Visit = {
  at: Date
  path: string
  country: string | null
  source?: string
  channel?: Channel
  referrer?: string | null
  query?: string | null
  utmSource?: string | null
  utmMedium?: string | null
  utmCampaign?: string | null
  device?: "mobile" | "tablet" | "desktop"
  agent?: string
  agentKind?: AgentKind
}

type Analytics = { recordVisit(visit: Visit): Promise<void> }

/** Referring hosts (or utm_source values) by name. The first match wins, so specific hosts come first. */
const SOURCES: [RegExp, string, Channel][] = [
  [/(^|\.)(chatgpt\.com|chat\.openai\.com)$|^chatgpt$|^openai$/, "ChatGPT", "ai"],
  [/(^|\.)perplexity\.ai$|^perplexity$/, "Perplexity", "ai"],
  [/(^|\.)claude\.ai$|^claude$/, "Claude", "ai"],
  [/^(gemini|bard)\.google\.com$|^gemini$/, "Gemini", "ai"],
  [/^copilot\.microsoft\.com$|(^|\.)copilot\.com$|^copilot$/, "Copilot", "ai"],
  [/(^|\.)deepseek\.com$/, "DeepSeek", "ai"],
  [/(^|\.)grok\.com$|^grok$/, "Grok", "ai"],
  [/(^|\.)meta\.ai$/, "Meta AI", "ai"],
  [/(^|\.)(you\.com|phind\.com)$/, "AI search", "ai"],
  [/^mail\.google\.com$|(^|\.)outlook\.(live|office)\.com$|^newsletter$|^email$/, "Email", "email"],
  [/(^|\.)google(\.[a-z]+)+$|^google$/, "Google", "search"],
  [/(^|\.)bing\.com$|^bing$/, "Bing", "search"],
  [/(^|\.)duckduckgo\.com$/, "DuckDuckGo", "search"],
  [/(^|\.)yahoo\.com$/, "Yahoo", "search"],
  [/(^|\.)yandex(\.[a-z]+)+$/, "Yandex", "search"],
  [/(^|\.)baidu\.com$/, "Baidu", "search"],
  [/(^|\.)ecosia\.org$/, "Ecosia", "search"],
  [/^search\.brave\.com$/, "Brave Search", "search"],
  [/(^|\.)(startpage\.com|kagi\.com|qwant\.com|naver\.com)$/, "Other search", "search"],
  [/^(t\.co|x\.com|twitter\.com)$|^(x|twitter)$/, "X", "social"],
  [/(^|\.)reddit\.com$|^reddit$/, "Reddit", "social"],
  [/^news\.ycombinator\.com$|^(hn|hackernews)$/, "Hacker News", "social"],
  [/(^|\.)linkedin\.com$|^lnkd\.in$|^linkedin$/, "LinkedIn", "social"],
  [/(^|\.)facebook\.com$|^fb\.me$/, "Facebook", "social"],
  [/(^|\.)instagram\.com$/, "Instagram", "social"],
  [/(^|\.)threads\.(net|com)$/, "Threads", "social"],
  [/^bsky\.app$/, "Bluesky", "social"],
  [/(^|\.)youtube\.com$|^youtu\.be$/, "YouTube", "social"],
  [/(^|\.)producthunt\.com$|^producthunt$/, "Product Hunt", "social"],
  [/(^|\.)indiehackers\.com$/, "Indie Hackers", "social"],
  [/(^|\.)github\.com$/, "GitHub", "referral"],
]

/** Agents by their user agent. AI assistants fetch a page while answering someone's question. */
const AGENTS: [RegExp, string, AgentKind][] = [
  [/ChatGPT-User/i, "ChatGPT", "assistant"],
  [/OAI-SearchBot/i, "ChatGPT search", "ai-search"],
  [/GPTBot/i, "GPTBot (OpenAI)", "ai-training"],
  [/Claude-User/i, "Claude", "assistant"],
  [/Claude-SearchBot/i, "Claude search", "ai-search"],
  [/ClaudeBot/i, "ClaudeBot (Anthropic)", "ai-training"],
  [/Perplexity-User/i, "Perplexity", "assistant"],
  [/PerplexityBot/i, "Perplexity search", "ai-search"],
  [/MistralAI-User/i, "Mistral Le Chat", "assistant"],
  [/DuckAssistBot/i, "DuckDuckGo AI", "assistant"],
  [/meta-externalfetcher/i, "Meta AI", "assistant"],
  [/meta-externalagent/i, "Meta AI crawler", "ai-training"],
  [/Google-CloudVertexBot|Gemini-Deep-Research/i, "Gemini", "assistant"],
  [/Bytespider/i, "Bytespider (ByteDance)", "ai-training"],
  [/CCBot/i, "Common Crawl", "ai-training"],
  [/Amazonbot/i, "Amazonbot", "ai-training"],
  [/Applebot/i, "Applebot", "search"],
  [/Googlebot/i, "Googlebot", "search"],
  [/bingbot/i, "Bingbot", "search"],
  [/DuckDuckBot/i, "DuckDuckBot", "search"],
  [/YandexBot/i, "YandexBot", "search"],
]

/** Other automated clients, which aren't counted at all. */
const BOT = /bot|crawl|spider|slurp|fetch|curl|wget|python|httpclient|axios|okhttp|java\/|go-http|headless|lighthouse|monitor|preview/i

/** Search terms some engines still put in the referring URL. */
const QUERY_PARAMS = ["q", "query", "p", "text", "wd", "searchfor"]

const clip = (value: string | null | undefined, max = 200) => (value ? value.trim().slice(0, max) || null : null)

function classify(host: string): [string, Channel] | null {
  const match = SOURCES.find(([pattern]) => pattern.test(host))
  return match ? [match[1], match[2]] : null
}

function device(ua: string): Visit["device"] {
  if (/iPad|Tablet/i.test(ua)) return "tablet"
  if (/Mobi|iPhone|Android/i.test(ua)) return "mobile"
  return "desktop"
}

/** What to record for this request and response, or null when it isn't a landing or an agent read. */
export function visitFor(request: Request, url: URL, response: Response): Visit | null {
  if (request.method !== "GET" || response.status !== 200) return null
  const type = response.headers.get("content-type") ?? ""
  const page = type.startsWith("text/html") || type.startsWith("text/markdown") || url.pathname.startsWith("/llms")
  if (!page) return null
  const ua = request.headers.get("user-agent") ?? ""
  const cf = (request as { cf?: { country?: string } }).cf
  const base = { at: new Date(), path: url.pathname, country: clip(cf?.country, 2) }

  const agent = AGENTS.find(([pattern]) => pattern.test(ua))
  if (agent) return { ...base, agent: agent[1], agentKind: agent[2] }

  // People: a browser loading a page (not a prefetch, not a script), arriving from elsewhere.
  if (BOT.test(ua) || request.headers.get("sec-fetch-dest") !== "document") return null
  if (/prefetch|prerender/.test(request.headers.get("sec-purpose") ?? request.headers.get("purpose") ?? "")) return null
  let referrer: URL | null = null
  try {
    const value = request.headers.get("referer")
    referrer = value ? new URL(value) : null
  } catch {}
  // Moving between pages, or coming back from the account pages on app.appdoctor.so.
  const home = url.hostname.replace(/^www\./, "")
  if (referrer && (referrer.hostname === home || referrer.hostname.endsWith(`.${home}`))) return null

  const params = url.searchParams
  const utmSource = clip(params.get("utm_source") ?? params.get("ref"), 100)
  const referrerHost = referrer?.hostname.replace(/^www\./, "") ?? null
  const known = (utmSource && classify(utmSource.toLowerCase())) || (referrerHost && classify(referrerHost))
  const [source, channel]: [string, Channel] = known
    ? known
    : utmSource
      ? [utmSource, "campaign"]
      : referrerHost
        ? [referrerHost, "referral"]
        : ["Direct", "direct"]
  const query = QUERY_PARAMS.map((p) => referrer?.searchParams.get(p)).find(Boolean) ?? params.get("utm_term")

  return {
    ...base,
    source,
    channel,
    referrer: clip(referrerHost, 100),
    query: clip(query),
    utmSource,
    utmMedium: clip(params.get("utm_medium"), 100),
    utmCampaign: clip(params.get("utm_campaign"), 100),
    device: device(ua),
  }
}

/** Records the visit after the response is sent. Locally there's no binding, so nothing is kept. */
export function recordVisit(request: Request, url: URL, response: Response) {
  const analytics = (env as { ANALYTICS?: Analytics }).ANALYTICS
  const visit = analytics && visitFor(request, url, response)
  if (!visit) return
  waitUntil(
    analytics.recordVisit(visit).catch((error) => {
      console.error("[web] recording a visit failed:", error instanceof Error ? error.message : error)
    }),
  )
}
