import { useEffect, useMemo, useState } from "react"
import {
  DEMO_APP,
  DEMO_AUDIT,
  DEMO_CHAT,
  DEMO_HISTORY,
  DEMO_KEYWORDS,
  DEMO_LISTING,
  type Country,
  type DemoKeyword,
} from "#/content/demo"

// AppDoctor's window, redrawn in HTML and CSS with example data, so people
// (and crawlers: it's all server-rendered text) can try the real screens.
// It follows the desktop app's dark look; every control does what it does
// in the app, on made-up numbers.

export type View = "audit" | "keywords" | "rankings" | "localization" | "screenshots"

export const VIEW_LABELS: Record<View, string> = {
  audit: "Audit",
  keywords: "Keywords",
  rankings: "Rankings",
  localization: "Localization",
  screenshots: "Screenshots",
}

const FLAGS: Record<Country, string> = { us: "🇺🇸", gb: "🇬🇧" }

export function AppWindow({ view, onView }: { view: View; onView: (view: View) => void }) {
  const [aiOpen, setAiOpen] = useState(false)
  return (
    <figure
      className="font-system overflow-hidden rounded-[14px] bg-app-window text-[13px] text-app-text shadow-[0_0_0_1px_rgb(0_0_0/0.35),0_40px_80px_-30px_rgb(15_27_45/0.55)]"
      aria-label="AppDoctor, with example data"
    >
      <TitleBar />
      <div className="flex flex-col md:h-[600px] md:flex-row">
        <Sidebar view={view} onView={onView} />
        <div className="flex min-h-[520px] min-w-0 flex-1 flex-col md:min-h-0">
          {view === "keywords" && <KeywordsView aiOpen={aiOpen} onAi={() => setAiOpen((o) => !o)} />}
          {view === "rankings" && <RankingsView />}
          {view === "audit" && <AuditView onAi={() => setAiOpen(true)} />}
          {view === "localization" && <LocalizationView aiOpen={aiOpen} onAi={() => setAiOpen((o) => !o)} />}
          {view === "screenshots" && <ScreenshotsView />}
        </div>
        {aiOpen && <AiPanel onClose={() => setAiOpen(false)} />}
      </div>
    </figure>
  )
}

function TitleBar() {
  return (
    <div className="flex h-10 items-center gap-3 border-b border-app-line bg-app-side px-4">
      <span className="flex gap-2" aria-hidden="true">
        <span className="size-3 rounded-full bg-[#ff5f57]" />
        <span className="size-3 rounded-full bg-[#febc2e]" />
        <span className="size-3 rounded-full bg-[#28c840]" />
      </span>
      <span className="flex items-center gap-1.5 font-semibold">
        <span className="grid size-[18px] place-items-center rounded-[5px] bg-app-accent">
          <Icon name="pulse" className="size-3 text-white" />
        </span>
        AppDoctor
      </span>
      <span className="ml-auto text-[11px] text-app-muted">Example data</span>
    </div>
  )
}

const NAV: { group: string; items: { view?: View; label: string; icon: IconName }[] }[] = [
  {
    group: "Optimize",
    items: [
      { view: "audit", label: "Audit", icon: "stethoscope" },
      { view: "keywords", label: "Keywords", icon: "search" },
      { view: "rankings", label: "Rankings", icon: "chart" },
      { label: "Competitors", icon: "users" },
    ],
  },
  {
    group: "Create",
    items: [
      { view: "localization", label: "Localization", icon: "languages" },
      { view: "screenshots", label: "Screenshots", icon: "image" },
    ],
  },
  { group: "Ship", items: [{ label: "App Review", icon: "check" }] },
]

function Sidebar({ view, onView }: { view: View; onView: (view: View) => void }) {
  return (
    <nav
      aria-label="AppDoctor sections"
      className="flex shrink-0 gap-1 overflow-x-auto border-b border-app-line bg-app-side p-2 md:w-[214px] md:flex-col md:overflow-visible md:border-r md:border-b-0 md:p-3"
    >
      <div className="mb-2 hidden items-center gap-2.5 rounded-lg bg-white/[0.06] p-2 md:flex">
        <AppIcon className="size-9" />
        <div className="min-w-0">
          <div className="truncate font-medium">{DEMO_APP.name}</div>
          <div className="text-[11px] text-app-muted">
            {FLAGS.us} {FLAGS.gb} iPhone · {DEMO_KEYWORDS.length} keywords
          </div>
        </div>
      </div>
      {NAV.map((group) => (
        <div key={group.group} className="contents md:block">
          <div className="mt-2 mb-1 hidden px-2 text-[11px] font-medium text-app-muted md:block">{group.group}</div>
          {group.items.map((item) => {
            const active = item.view === view
            const content = (
              <>
                <Icon name={item.icon} className={`size-[15px] ${active ? "text-app-accent-soft" : "text-app-muted"}`} />
                {item.label}
              </>
            )
            const classes = "flex h-[30px] shrink-0 items-center gap-2 rounded-md px-2 whitespace-nowrap"
            return item.view ? (
              <button
                key={item.label}
                type="button"
                aria-pressed={active}
                onClick={() => onView(item.view!)}
                className={`${classes} ${active ? "bg-white/[0.09] font-medium" : "hover:bg-white/[0.05]"}`}
              >
                {content}
              </button>
            ) : (
              <span key={item.label} className={`${classes} hidden text-app-muted/70 md:flex`}>
                {content}
              </span>
            )
          })}
        </div>
      ))}
    </nav>
  )
}

function ViewHeader({ title, sub, children }: { title: string; sub: string; children?: React.ReactNode }) {
  return (
    <div className="flex min-h-[56px] flex-wrap items-center gap-3 border-b border-app-line px-4 py-2">
      <AppIcon className="hidden size-8 sm:block" />
      <div className="min-w-0 flex-1">
        <div className="truncate text-[14px] font-semibold">{title}</div>
        <div className="truncate text-[11px] text-app-muted">{sub}</div>
      </div>
      {children}
    </div>
  )
}

function AiButton({ open, onClick }: { open: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      aria-pressed={open}
      onClick={onClick}
      className={`flex h-7 items-center gap-1.5 rounded-md px-2.5 font-medium ${open ? "bg-app-accent text-white" : "text-app-accent-soft hover:bg-white/[0.06]"}`}
    >
      <Icon name="sparkles" className="size-3.5" />
      AI
    </button>
  )
}

/* ------------------------------------------------------------------------ */
/* Keywords                                                                  */
/* ------------------------------------------------------------------------ */

type SortKey = "term" | "popularity" | "difficulty" | "rank" | "change"

/** A stable 0–1 number from a word, so added keywords get the same example values every time. */
function hash(text: string, salt: number) {
  let h = 2166136261 ^ salt
  for (const c of text) h = Math.imul(h ^ c.charCodeAt(0), 16777619)
  return ((h >>> 0) % 1000) / 1000
}

function exampleKeyword(term: string): DemoKeyword {
  const popularity = 5 + Math.round(hash(term, 1) * 55)
  const difficulty = 10 + Math.round(hash(term, 2) * 70)
  const rank = (salt: number) => (hash(term, salt) > 0.35 ? 1 + Math.round(hash(term, salt + 7) * 90) : null)
  return {
    term,
    popularity,
    difficulty,
    rank: { us: { now: rank(3), before: null }, gb: { now: rank(4), before: null } },
    top: ["#6366f1", "#0ea5e9", "#f59e0b"],
  }
}

function change(k: DemoKeyword, country: Country): number | null {
  const { now, before } = k.rank[country]
  if (now == null || before == null) return null
  return before - now
}

function KeywordsView({ aiOpen, onAi }: { aiOpen: boolean; onAi: () => void }) {
  const [country, setCountry] = useState<Country>("us")
  const [filter, setFilter] = useState("")
  const [added, setAdded] = useState<DemoKeyword[]>([])
  const [checking, setChecking] = useState<string | null>(null)
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean }>({ key: "popularity", desc: true })

  const all = useMemo(() => [...added, ...DEMO_KEYWORDS], [added])
  const query = filter.trim().toLowerCase().replace(/\s+/g, " ")
  const rows = useMemo(() => {
    const value = (k: DemoKeyword): number | string => {
      switch (sort.key) {
        case "term":
          return k.term
        case "popularity":
          return k.popularity
        case "difficulty":
          return k.difficulty
        case "rank":
          return k.rank[country].now ?? 999
        case "change":
          return change(k, country) ?? -999
      }
    }
    return all
      .filter((k) => k.term.includes(query))
      .sort((a, b) => {
        const [x, y] = [value(a), value(b)]
        const order = typeof x === "string" ? x.localeCompare(y as string) : (x as number) - (y as number)
        return sort.desc ? -order : order
      })
  }, [all, query, sort, country])

  const canAdd = query.length > 1 && !all.some((k) => k.term === query)
  function add() {
    if (!canAdd) return
    setAdded((list) => [exampleKeyword(query), ...list])
    setChecking(query)
    setFilter("")
  }
  useEffect(() => {
    if (!checking) return
    const t = setTimeout(() => setChecking(null), 900)
    return () => clearTimeout(t)
  }, [checking])

  const ranked = all.map((k) => k.rank[country].now)
  const buckets = [
    ["top 10", ranked.filter((r) => r != null && r <= 10).length],
    ["top 11–50", ranked.filter((r) => r != null && r > 10 && r <= 50).length],
    ["ranked 51+", ranked.filter((r) => r != null && r > 50).length],
    ["not ranked", ranked.filter((r) => r == null).length],
  ] as const

  const header = (key: SortKey, label: string, className = "") => {
    const active = sort.key === key
    return (
      <th scope="col" aria-sort={active ? (sort.desc ? "descending" : "ascending") : "none"} className={`px-3 py-2 font-medium ${className}`}>
        <button
          type="button"
          onClick={() => setSort((s) => ({ key, desc: s.key === key ? !s.desc : key !== "term" }))}
          className={`inline-flex items-center gap-1 ${active ? "text-app-text" : "hover:text-app-text"}`}
        >
          {label}
          <span aria-hidden="true" className={active ? "" : "opacity-30"}>
            {active && !sort.desc ? "↑" : "↓"}
          </span>
        </button>
      </th>
    )
  }

  return (
    <>
      <ViewHeader title={DEMO_APP.name} sub={`${all.length} keywords in ${FLAGS[country]} ${country.toUpperCase()} · checked 2 hours ago`}>
        <div className="flex rounded-md bg-white/[0.06] p-0.5" role="group" aria-label="Country">
          {DEMO_APP.countries.map((c) => (
            <button
              key={c}
              type="button"
              aria-pressed={c === country}
              onClick={() => setCountry(c)}
              className={`rounded px-2 py-0.5 text-[12px] ${c === country ? "bg-white/[0.14] font-medium" : "text-app-muted"}`}
            >
              {FLAGS[c]} {c.toUpperCase()}
            </button>
          ))}
        </div>
        <form
          className="flex items-center gap-1.5"
          onSubmit={(e) => {
            e.preventDefault()
            add()
          }}
        >
          <label className="flex h-7 w-40 items-center gap-1.5 rounded-md bg-white/[0.06] px-2 focus-within:ring-1 focus-within:ring-app-accent">
            <Icon name="search" className="size-3.5 text-app-muted" />
            <span className="sr-only">Filter or add a keyword</span>
            <input
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Filter or add…"
              className="w-full bg-transparent text-[12px] outline-none placeholder:text-app-muted"
            />
          </label>
          <button type="submit" disabled={!canAdd} className="h-7 rounded-md bg-app-accent px-2.5 font-medium text-white disabled:opacity-40">
            Add
          </button>
        </form>
        <AiButton open={aiOpen} onClick={onAi} />
      </ViewHeader>
      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full text-left tabular-nums">
          <caption className="sr-only">Example keywords with Apple popularity, difficulty and App Store rank</caption>
          <thead className="sticky top-0 bg-app-window text-[11px] text-app-muted">
            <tr className="border-b border-app-line">
              {header("term", "Keyword", "pl-4")}
              {header("popularity", "Popularity")}
              {header("difficulty", "Difficulty", "hidden sm:table-cell")}
              {header("rank", "Position")}
              {header("change", "Change")}
              <th scope="col" className="hidden px-3 py-2 font-medium lg:table-cell">
                Apps in ranking
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map((k) => {
              const rank = k.rank[country].now
              const moved = change(k, country)
              const isChecking = checking === k.term
              return (
                <tr key={k.term} className={`border-b border-app-line/60 ${isChecking ? "bg-app-accent/10" : "hover:bg-white/[0.03]"}`}>
                  <td className="py-2 pr-3 pl-4">{k.term}</td>
                  <td className="px-3 py-2">
                    <Meter value={k.popularity} color={k.popularity >= 50 ? "#34c77b" : k.popularity >= 25 ? "#f2b84b" : "#8a8a96"} />
                  </td>
                  <td className="hidden px-3 py-2 sm:table-cell">
                    <Meter value={k.difficulty} color={k.difficulty >= 70 ? "#ff6b6b" : k.difficulty >= 45 ? "#f2b84b" : "#34c77b"} />
                  </td>
                  <td className="px-3 py-2 font-medium">{isChecking ? <span className="text-app-muted">Checking…</span> : (rank ?? <span className="text-app-muted">—</span>)}</td>
                  <td className="px-3 py-2">
                    {moved == null || isChecking ? (
                      <span className="text-app-muted">—</span>
                    ) : moved > 0 ? (
                      <span className="text-app-up">▲ {moved}</span>
                    ) : moved < 0 ? (
                      <span className="text-app-down">▼ {-moved}</span>
                    ) : (
                      <span className="text-app-muted">0</span>
                    )}
                  </td>
                  <td className="hidden px-3 py-2 lg:table-cell">
                    <span className="flex gap-1" aria-hidden="true">
                      {k.top.map((color, i) => (
                        <span key={i} className="size-4 rounded-[4px]" style={{ background: color }} />
                      ))}
                    </span>
                  </td>
                </tr>
              )
            })}
            {rows.length === 0 && (
              <tr>
                <td colSpan={6} className="px-4 py-6 text-app-muted">
                  No keyword matches “{filter.trim()}”. Press Add to track it.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      <div className="flex flex-wrap gap-x-4 gap-y-1 border-t border-app-line px-4 py-2 text-[11px] text-app-muted">
        {buckets.map(([label, n]) => (
          <span key={label}>
            <span className="font-semibold text-app-text">{n}</span> {label}
          </span>
        ))}
      </div>
    </>
  )
}

function Meter({ value, color }: { value: number; color: string }) {
  return (
    <span className="flex items-center gap-2">
      <span className="w-5 text-right font-medium">{value}</span>
      <span className="hidden h-[5px] w-12 overflow-hidden rounded-full bg-white/[0.08] sm:block" aria-hidden="true">
        <span className="block h-full rounded-full" style={{ width: `${Math.max(value, 4)}%`, background: color }} />
      </span>
    </span>
  )
}

/* ------------------------------------------------------------------------ */
/* Rankings                                                                  */
/* ------------------------------------------------------------------------ */

function RankingsView() {
  const [hidden, setHidden] = useState<string[]>([])
  const width = 600
  const height = 260
  const worst = 60
  const x = (i: number) => 16 + (i / 29) * (width - 32)
  const y = (rank: number) => 14 + ((rank - 1) / (worst - 1)) * (height - 40)
  return (
    <>
      <ViewHeader title="Rankings" sub={`${FLAGS.us} US · last 30 days`} />
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-auto p-4">
        <div className="flex flex-wrap gap-2" role="group" aria-label="Keywords shown">
          {DEMO_HISTORY.map((line) => {
            const shown = !hidden.includes(line.term)
            return (
              <button
                key={line.term}
                type="button"
                aria-pressed={shown}
                onClick={() => setHidden((h) => (shown ? [...h, line.term] : h.filter((t) => t !== line.term)))}
                className={`flex items-center gap-1.5 rounded-md px-2 py-1 ${shown ? "bg-white/[0.08]" : "text-app-muted"}`}
              >
                <span className="size-2 rounded-full" style={{ background: shown ? line.color : "transparent", boxShadow: `inset 0 0 0 1.5px ${line.color}` }} />
                {line.term}
                <span className="text-app-muted">
                  {line.ranks[0]} → {line.ranks[29]}
                </span>
              </button>
            )
          })}
        </div>
        <svg viewBox={`0 0 ${width} ${height}`} className="w-full" role="img" aria-label="Rank history: sleep timer from 22 to 8, rain sounds from 30 to 19, sleep sounds from 58 to 34">
          {[1, 10, 20, 30, 40, 50, 60].map((rank) => (
            <g key={rank}>
              <line x1={16} x2={width - 16} y1={y(rank)} y2={y(rank)} stroke="rgb(255 255 255 / 0.06)" />
              <text x={width - 14} y={y(rank) - 3} textAnchor="end" fontSize="9" fill="rgb(255 255 255 / 0.35)">
                #{rank}
              </text>
            </g>
          ))}
          {DEMO_HISTORY.filter((line) => !hidden.includes(line.term)).map((line) => (
            <g key={line.term}>
              <polyline
                points={line.ranks.map((rank, i) => `${x(i)},${y(rank)}`).join(" ")}
                fill="none"
                stroke={line.color}
                strokeWidth="2"
                strokeLinejoin="round"
                strokeLinecap="round"
              />
              <circle cx={x(29)} cy={y(line.ranks[29]!)} r="3.5" fill={line.color} />
            </g>
          ))}
          <text x={16} y={height - 6} fontSize="10" fill="rgb(255 255 255 / 0.4)">
            30 days ago
          </text>
          <text x={width - 16} y={height - 6} fontSize="10" textAnchor="end" fill="rgb(255 255 255 / 0.4)">
            Today
          </text>
        </svg>
        <p className="text-[12px] text-app-muted">Higher is better. The release on day 12 added “timer” to the subtitle.</p>
      </div>
    </>
  )
}

/* ------------------------------------------------------------------------ */
/* Audit                                                                     */
/* ------------------------------------------------------------------------ */

function scoreColor(score: number) {
  return score >= 90 ? "#34c77b" : score >= 75 ? "#7c7cff" : score >= 55 ? "#f2b84b" : "#ff6b6b"
}

function AuditView({ onAi }: { onAi: () => void }) {
  return (
    <>
      <ViewHeader title="ASO audit" sub={`${DEMO_APP.name} · checked just now`}>
        <button type="button" onClick={onAi} className="flex h-7 items-center gap-1.5 rounded-md bg-app-accent px-2.5 font-medium text-white">
          <Icon name="sparkles" className="size-3.5" />
          Fix with AI
        </button>
      </ViewHeader>
      <div className="min-h-0 flex-1 overflow-auto p-4">
        <div className="flex flex-col gap-5 rounded-xl bg-white/[0.04] p-5 ring-1 ring-app-line sm:flex-row sm:items-center sm:gap-8">
          <div className="shrink-0">
            <div className="flex items-baseline gap-1">
              <span className="text-[52px] leading-none font-bold" style={{ color: scoreColor(DEMO_AUDIT.score) }}>
                {DEMO_AUDIT.score}
              </span>
              <span className="text-[15px] text-app-muted">/100</span>
            </div>
            <div className="mt-1 text-[15px] font-semibold">{DEMO_AUDIT.grade}</div>
            <div className="text-[12px] text-app-muted">3 to fix, 5 tips</div>
          </div>
          <dl className="flex flex-1 flex-col gap-2.5">
            {DEMO_AUDIT.areas.map((area) => (
              <div key={area.label} className="flex items-center gap-3">
                <dt className="w-40 shrink-0 text-[12px]">{area.label}</dt>
                <dd className="flex flex-1 items-center gap-3">
                  <span className="h-[6px] flex-1 overflow-hidden rounded-full bg-white/[0.08]">
                    <span className="block h-full rounded-full" style={{ width: `${area.score}%`, background: scoreColor(area.score) }} />
                  </span>
                  <span className="w-6 text-right text-[12px] font-medium">{area.score}</span>
                </dd>
              </div>
            ))}
          </dl>
        </div>
        <h3 className="mt-5 mb-2 font-semibold">Fix first</h3>
        <ul className="divide-y divide-app-line rounded-xl bg-white/[0.04] ring-1 ring-app-line">
          {DEMO_AUDIT.findings.map((f) => (
            <li key={f.title} className="flex gap-3 px-4 py-3">
              <Icon name={f.level === "warning" ? "warning" : "info"} className={`mt-0.5 size-4 shrink-0 ${f.level === "warning" ? "text-app-warn" : "text-app-accent-soft"}`} />
              <div>
                <div className="font-semibold">{f.title}</div>
                <div className="mt-0.5 text-[12px] text-app-muted">{f.detail}</div>
              </div>
            </li>
          ))}
        </ul>
      </div>
    </>
  )
}

/* ------------------------------------------------------------------------ */
/* Localization                                                              */
/* ------------------------------------------------------------------------ */

function wordsOf(text: string) {
  return text
    .toLowerCase()
    .split(/[^\p{L}\p{N}]+/u)
    .filter(Boolean)
}

function LocalizationView({ aiOpen, onAi }: { aiOpen: boolean; onAi: () => void }) {
  const [listing, setListing] = useState(DEMO_LISTING)
  const [locale, setLocale] = useState<"en-US" | "en-GB">("en-US")
  const indexed = new Set(wordsOf(`${listing.name} ${listing.subtitle} ${listing.keywords}`))
  const covered = DEMO_KEYWORDS.filter((k) => k.term.split(" ").every((w) => indexed.has(w)))
  const missing = DEMO_KEYWORDS.filter((k) => !covered.includes(k))
  const shown = new Set(wordsOf(`${listing.name} ${listing.subtitle}`))
  const seen = new Set<string>()
  const repeated = [...new Set(wordsOf(listing.keywords).filter((w) => shown.has(w) || (seen.has(w) ? true : (seen.add(w), false))))]
  const spaced = /,\s|\s,/.test(listing.keywords)

  const field = (key: keyof typeof listing, label: string, limit: number) => {
    const length = [...listing[key]].length
    return (
      <label className="block">
        <span className="mb-1 flex justify-between text-[12px]">
          <span className="font-medium">{label}</span>
          <span className={length > limit ? "font-semibold text-app-down" : "text-app-muted"}>
            {length}/{limit}
          </span>
        </span>
        <input
          value={listing[key]}
          onChange={(e) => setListing((l) => ({ ...l, [key]: e.target.value }))}
          spellCheck={false}
          className={`h-8 w-full rounded-md bg-white/[0.06] px-2.5 outline-none focus:ring-1 ${length > limit ? "ring-1 ring-app-down" : "focus:ring-app-accent"}`}
        />
      </label>
    )
  }

  return (
    <>
      <ViewHeader title="Localization" sub={`${DEMO_APP.name} · version 2.4 (Prepare for Submission)`}>
        <AiButton open={aiOpen} onClick={onAi} />
      </ViewHeader>
      <div className="flex min-h-0 flex-1">
        <ul className="hidden w-44 shrink-0 flex-col gap-0.5 border-r border-app-line p-2 sm:flex" aria-label="Languages">
          {(["en-US", "en-GB"] as const).map((code) => (
            <li key={code}>
              <button
                type="button"
                aria-pressed={locale === code}
                onClick={() => setLocale(code)}
                className={`w-full rounded-md px-2 py-1.5 text-left ${locale === code ? "bg-app-accent text-white" : "hover:bg-white/[0.05]"}`}
              >
                <span className="flex items-center gap-1.5">
                  {code === "en-US" ? "English (U.S.)" : "English (U.K.)"}
                  {code === "en-GB" && <Icon name="sparkles" className="size-3" />}
                </span>
                <span className={`block text-[11px] ${locale === code ? "text-white/75" : "text-app-muted"}`}>
                  {code === "en-US" ? listing.name : "Suggested by your AI"}
                </span>
              </button>
            </li>
          ))}
        </ul>
        <div className="min-w-0 flex-1 overflow-auto p-4">
          {locale === "en-GB" ? (
            <div className="rounded-lg bg-app-accent/15 p-4 ring-1 ring-app-accent/40">
              <div className="flex items-center gap-2 font-semibold">
                <Icon name="sparkles" className="size-4 text-app-accent-soft" />
                Your AI wrote an English (U.K.) listing
              </div>
              <p className="mt-1 text-[12px] text-app-muted">
                You track Moonwake in the UK but its listing has no English (U.K.). Add the language to App Store Connect, review the suggestion in the fields, then save.
              </p>
              <dl className="mt-3 grid gap-1 text-[12px]">
                <div className="flex gap-2">
                  <dt className="w-20 text-app-muted">Name</dt>
                  <dd>Moonwake: Sleep Sounds</dd>
                </div>
                <div className="flex gap-2">
                  <dt className="w-20 text-app-muted">Subtitle</dt>
                  <dd>Rain, sea &amp; brown noise</dd>
                </div>
                <div className="flex gap-2">
                  <dt className="w-20 text-app-muted">Keywords</dt>
                  <dd className="break-all">timer,white,baby,lullaby,nap,fan,calm,night,relax,stories,waves,ocean</dd>
                </div>
              </dl>
            </div>
          ) : (
            <div className="flex flex-col gap-3">
              {field("name", "Name", 30)}
              {field("subtitle", "Subtitle", 30)}
              {field("keywords", "Keywords", 100)}
              <div className="rounded-lg bg-white/[0.04] p-3 ring-1 ring-app-line" aria-live="polite">
                <div className="text-[12px] font-medium">
                  Covers {covered.length} of {DEMO_KEYWORDS.length} tracked keywords
                </div>
                {missing.length > 0 && (
                  <div className="mt-2 flex flex-wrap gap-1.5">
                    {missing.map((k) => (
                      <span key={k.term} className="rounded bg-app-down/15 px-1.5 py-0.5 text-[11px] text-app-down">
                        {k.term}
                      </span>
                    ))}
                  </div>
                )}
                {repeated.length > 0 && (
                  <p className="mt-2 text-[12px] text-app-warn">
                    Repeated: {repeated.join(", ")}. Apple counts each word once, so these characters could hold new keywords.
                  </p>
                )}
                {spaced && <p className="mt-2 text-[12px] text-app-warn">Spaces after commas use characters search doesn't need.</p>}
                {missing.length === 0 && repeated.length === 0 && !spaced && (
                  <p className="mt-2 text-[12px] text-app-up">Every tracked keyword is searchable, with no wasted characters.</p>
                )}
              </div>
              <p className="text-[11px] text-app-muted">Try it: take out “sleep”, “noise” and “sounds”, add “stories” and “waves”, and watch the coverage change.</p>
            </div>
          )}
        </div>
      </div>
    </>
  )
}

/* ------------------------------------------------------------------------ */
/* Screenshots                                                               */
/* ------------------------------------------------------------------------ */

const SLIDES = [
  { headline: "Fall asleep in minutes", sub: "Rain, ocean and brown noise", from: "#3b2a8f", to: "#7c3aed" },
  { headline: "Mix your own night", sub: "Layer up to five sounds", from: "#0f3b6e", to: "#0891b2" },
  { headline: "Wake up gently", sub: "A sunrise alarm that fades in", from: "#7c2d12", to: "#f59e0b" },
]

function ScreenshotsView() {
  return (
    <>
      <ViewHeader title="iPhone screenshots" sub={`${DEMO_APP.name} · 3 screenshots · 1320 × 2868`} />
      <div className="flex min-h-0 flex-1 items-center justify-center overflow-auto bg-black/20 p-5">
        <div className="flex gap-3">
          {SLIDES.map((slide) => (
            <div
              key={slide.headline}
              className="flex aspect-[1320/2868] w-[clamp(84px,14vw,150px)] flex-col items-center rounded-[10px] px-2 pt-4 text-center"
              style={{ background: `linear-gradient(170deg, ${slide.from}, ${slide.to})` }}
            >
              <div className="text-[clamp(10px,1.3vw,14px)] leading-tight font-bold text-white">{slide.headline}</div>
              <div className="mt-1 text-[clamp(7px,0.8vw,9px)] text-white/75">{slide.sub}</div>
              <div className="mt-auto mb-[-1px] h-[58%] w-[78%] rounded-t-[14px] border-[3px] border-b-0 border-[#0b0b0f] bg-[#16161c] p-1.5">
                <div className="mx-auto h-1.5 w-8 rounded-full bg-black" />
                <div className="mt-2 h-2 w-3/4 rounded bg-white/20" />
                <div className="mt-1.5 h-2 w-1/2 rounded bg-white/10" />
                <div className="mt-3 aspect-square w-full rounded-lg bg-white/[0.07]" />
              </div>
            </div>
          ))}
        </div>
      </div>
      <p className="border-t border-app-line px-4 py-2 text-[11px] text-app-muted">
        Every iPhone and iPad frame, the iOS 27 product page header and search art, and app preview videos at Apple's sizes.
      </p>
    </>
  )
}

/* ------------------------------------------------------------------------ */
/* AI panel                                                                  */
/* ------------------------------------------------------------------------ */

function AiPanel({ onClose }: { onClose: () => void }) {
  // The conversation plays out once when the panel opens.
  const [shown, setShown] = useState(0)
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setShown(DEMO_CHAT.steps.length + 1)
      return
    }
    const timers = Array.from({ length: DEMO_CHAT.steps.length + 1 }, (_, i) => setTimeout(() => setShown(i + 1), 450 * (i + 1)))
    return () => timers.forEach(clearTimeout)
  }, [])
  return (
    <aside aria-label="Make with AI" className="flex w-full shrink-0 flex-col border-t border-app-line bg-app-side md:w-[290px] md:border-t-0 md:border-l">
      <div className="flex items-center justify-between border-b border-app-line px-4 py-3">
        <span className="text-[14px] font-semibold">Make with AI</span>
        <button type="button" onClick={onClose} aria-label="Close" className="rounded px-1.5 text-app-muted hover:bg-white/[0.06]">
          ✕
        </button>
      </div>
      <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-auto p-4" aria-live="polite">
        <div className="ml-8 self-end rounded-2xl rounded-br-[4px] bg-app-accent px-3 py-2 text-white">{DEMO_CHAT.asked}</div>
        <ul className="flex flex-col gap-1.5 text-[12px] text-app-muted">
          {DEMO_CHAT.steps.slice(0, shown).map((step) => (
            <li key={step} className="flex items-center gap-2">
              <Icon name="check" className="size-3.5 text-app-up" />
              {step}
            </li>
          ))}
        </ul>
        {shown > DEMO_CHAT.steps.length && <p className="leading-relaxed">{DEMO_CHAT.reply}</p>}
      </div>
      <div className="border-t border-app-line p-3">
        <div className="rounded-xl bg-white/[0.05] p-2 ring-1 ring-app-line">
          <div className="px-1 pb-2 text-app-muted">Ask about your listing, keywords…</div>
          <div className="flex items-center gap-2 text-[11px] text-app-muted">
            <span className="flex items-center gap-1 rounded px-1.5 py-0.5 hover:bg-white/[0.06]">
              <Icon name="folder" className="size-3" /> moonwake
            </span>
            <span className="flex items-center gap-1 rounded px-1.5 py-0.5">
              <span className="size-2 rounded-full bg-[#d97757]" /> Claude Code
            </span>
            <span className="ml-auto grid size-5 place-items-center rounded-full bg-app-accent text-white">↑</span>
          </div>
        </div>
      </div>
    </aside>
  )
}

/* ------------------------------------------------------------------------ */
/* Icons                                                                     */
/* ------------------------------------------------------------------------ */

function AppIcon({ className = "" }: { className?: string }) {
  return (
    <span className={`grid shrink-0 place-items-center rounded-[22%] bg-[linear-gradient(160deg,#2b2370,#6d4bd8)] ${className}`} aria-hidden="true">
      <svg viewBox="0 0 24 24" className="size-[58%]">
        <path d="M15.5 4.5a7.5 7.5 0 1 0 4 13.6A8.5 8.5 0 0 1 15.5 4.5Z" fill="#f7f3ff" />
      </svg>
    </span>
  )
}

type IconName = "search" | "chart" | "users" | "languages" | "image" | "check" | "stethoscope" | "sparkles" | "warning" | "info" | "pulse" | "folder"

const PATHS: Record<IconName, string> = {
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14Zm5-2 4 4",
  chart: "M4 19V5m0 14h16M8 15l3-4 3 2 5-6",
  users: "M16 19v-1a4 4 0 0 0-4-4H7a4 4 0 0 0-4 4v1m6.5-9a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7ZM21 19v-1a4 4 0 0 0-3-3.9M16 3.1a3.5 3.5 0 0 1 0 6.8",
  languages: "M4 5h9M8.5 3v2m2.5 0c-1 4-3.5 7-7 8.5M6 9c1.2 2 3 3.5 5 4.5M13 21l4-10 4 10m-7-3h6",
  image: "M5 4h14a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1Zm-1 12 4.5-4.5L13 16m-1-1 2.5-2.5L20 18M15 9.5a1.5 1.5 0 1 0 0-.01",
  check: "M20 6 9 17l-5-5",
  stethoscope: "M6 3v6a5 5 0 0 0 10 0V3M6 3H4m2 0h2m8 0h-2m2 0h2M11 14v2a5 5 0 0 0 10 0v-2m0 0a2 2 0 1 0 0-.01",
  sparkles: "M12 3l1.8 4.7L18.5 9.5l-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8L12 3Zm6.5 11 .9 2.1 2.1.9-2.1.9-.9 2.1-.9-2.1-2.1-.9 2.1-.9.9-2.1Z",
  warning: "M12 4 2.8 20h18.4L12 4Zm0 6v4m0 3v.01",
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm0-10v6m0-9v.01",
  pulse: "M3 12h4l2-5 4 10 2-5h6",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z",
}

function Icon({ name, className = "" }: { name: IconName; className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d={PATHS[name]} />
    </svg>
  )
}
