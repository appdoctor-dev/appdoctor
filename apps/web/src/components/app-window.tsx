import { useEffect, useMemo, useRef, useState } from "react"
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

/**
 * The window, starting on `start` (and with the AI panel open, given `ai`).
 * Each copy keeps its own state, so every section of a page can show a
 * different screen that still works.
 */
export function AppWindow({ start = "keywords", ai = false, height = 600, label }: { start?: View; ai?: boolean; height?: number; label?: string }) {
  const [view, onView] = useState<View>(start)
  const [aiOpen, setAiOpen] = useState(ai)
  // A chat open from the start is shown whole (and server-rendered); one
  // the reader opens plays out.
  const opened = useRef(false)
  const toggleAi = (open?: boolean) => {
    opened.current = true
    setAiOpen((o) => open ?? !o)
  }
  return (
    <figure
      className="font-system overflow-hidden rounded-[14px] bg-app-window text-left text-[13px] text-app-text shadow-[0_0_0_0.5px_rgb(0_0_0/0.4),0_30px_70px_-20px_rgb(0_0_0/0.35)]"
      aria-label={label ?? "AppDoctor, with example data"}
    >
      <TitleBar />
      <div className="flex flex-col md:h-[var(--app-height)] md:flex-row" style={{ "--app-height": `${height}px` } as React.CSSProperties}>
        <Sidebar view={view} onView={onView} />
        <div className="flex min-h-[520px] min-w-0 flex-1 flex-col md:min-h-0">
          {view === "keywords" && <KeywordsView aiOpen={aiOpen} onAi={() => toggleAi()} />}
          {view === "rankings" && <RankingsView />}
          {view === "audit" && <AuditView onAi={() => toggleAi(true)} />}
          {view === "localization" && <LocalizationView aiOpen={aiOpen} onAi={() => toggleAi()} />}
          {view === "screenshots" && <ScreenshotsView />}
        </div>
        {aiOpen && view !== "screenshots" && <AiPanel play={opened.current} onClose={() => setAiOpen(false)} />}
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
        <img src="/favicon.svg" alt="" className="size-[18px]" />
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
  {
    headline: "Fall asleep in minutes",
    sub: "Rain, ocean and brown noise",
    from: "#2a1d78",
    to: "#7c3aed",
    glow: "#c4b5fd",
    screen: "sleep",
  },
  {
    headline: "Mix your own night",
    sub: "Layer up to five sounds",
    from: "#0b2f5c",
    to: "#0891b2",
    glow: "#67e8f9",
    screen: "mix",
  },
  {
    headline: "Wake up gently",
    sub: "A sunrise alarm that fades in",
    from: "#6b1d0c",
    to: "#f59e0b",
    glow: "#fde68a",
    screen: "alarm",
  },
] as const

type Slide = (typeof SLIDES)[number]

// A live demo, timed in ms: the reader watches someone pick the project and
// model, type a request and send it; Claude answers, then designs the set one
// slide after another while the chat says what it's doing.
const DESIGN = {
  asked: "Design three iPhone screenshots for Moonwake. Calm, night-time, and use my best keywords.",
  intro: "On it. I'll build three slides around your strongest keywords.",
  steps: [
    { at: 7900, text: "Read 12 keywords and the App Store listing" },
    { at: 8500, text: "Picked a night palette: violet, ocean, sunrise" },
    { at: 11000, text: "Designed 1: Fall asleep in minutes" },
    { at: 13200, text: "Designed 2: Mix your own night" },
    { at: 15400, text: "Designed 3: Wake up gently" },
  ],
  reply:
    "Done. The headlines lead with “sleep sounds” and “white noise”, your most popular keywords, and each phone shows the screen it talks about. Want an iPad set next?",
  // The reader's side: the cursor's stops, and when the text is typed and sent.
  cursor: [
    { at: 400, to: "folder" },
    { at: 1150, to: "folderItem", click: true },
    { at: 1900, to: "model" },
    { at: 2650, to: "modelItem", click: true },
    { at: 3300, to: "input" },
    { at: 6000, to: "send", click: true },
  ] as const,
  folderMenu: [900, 1500] as const,
  modelMenu: [2350, 2950] as const,
  typeAt: 3500,
  sendAt: 6250,
  introAt: 6700,
  slideAt: (i: number) => 9000 + i * 2200,
  replyAt: 15900,
  end: 18300,
}

type Target = (typeof DESIGN.cursor)[number]["to"]

/** Milliseconds into the demo; it plays the first time the window scrolls into view. */
function useDemoClock(end: number) {
  const ref = useRef<HTMLDivElement>(null)
  // Server-rendered (and for reduced motion) as the finished design.
  const [ms, setMs] = useState(Number.POSITIVE_INFINITY)
  const [run, setRun] = useState(0)
  useEffect(() => {
    const node = ref.current
    if (!node || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return
    let frame = 0
    const play = () => {
      const start = performance.now()
      const tick = (now: number) => {
        const elapsed = now - start
        setMs(Math.round(elapsed / 50) * 50)
        if (elapsed < end) frame = requestAnimationFrame(tick)
      }
      frame = requestAnimationFrame(tick)
    }
    if (run > 0) {
      play()
      return () => cancelAnimationFrame(frame)
    }
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry?.isIntersecting) return
        observer.disconnect()
        play()
      },
      { threshold: 0.45 },
    )
    observer.observe(node)
    return () => {
      observer.disconnect()
      cancelAnimationFrame(frame)
    }
  }, [end, run])
  return { ref, ms, done: ms >= end, replay: () => setRun((r) => r + 1) }
}

function ScreenshotsView() {
  const { ref, ms, done, replay } = useDemoClock(DESIGN.end)
  const active = SLIDES.findIndex((_, i) => ms >= DESIGN.slideAt(i) && ms < DESIGN.slideAt(i + 1) && i < SLIDES.length)
  return (
    <div ref={ref} className="flex min-h-0 flex-1 flex-col lg:flex-row">
      <div className="flex min-h-0 min-w-0 flex-1 flex-col">
        <ViewHeader title="iPhone screenshots" sub={`${DEMO_APP.name} · 3 screenshots · 1320 × 2868`}>
          {done && Number.isFinite(ms) && (
            <button type="button" onClick={replay} className="h-7 rounded-md px-2.5 text-app-accent-soft hover:bg-white/[0.06]">
              Replay
            </button>
          )}
        </ViewHeader>
        <div className="relative flex min-h-[340px] flex-1 items-center justify-center overflow-hidden bg-[#141417] p-5 [background-image:radial-gradient(rgb(255_255_255/0.06)_1px,transparent_1px)] [background-size:18px_18px]">
          <div className="flex gap-[clamp(8px,1.4vw,16px)]">
            {SLIDES.map((slide, i) => (
              <SlideCard key={slide.headline} slide={slide} t={ms - DESIGN.slideAt(i)} editing={i === active} />
            ))}
          </div>
          {ms >= DESIGN.sendAt && ms < DESIGN.replyAt && (
            <div className="absolute top-3 left-1/2 flex -translate-x-1/2 items-center gap-2 rounded-full bg-black/60 px-3 py-1 text-[11px] text-app-text ring-1 ring-white/10 backdrop-blur lg:hidden">
              <img src="/claude.svg" alt="" className="size-3.5" /> Claude is designing…
            </div>
          )}
        </div>
        <p className="border-t border-app-line px-4 py-2 text-[11px] text-app-muted">
          Every iPhone and iPad size, the iOS 27 header and search art, and app preview videos.
        </p>
      </div>
      <DesignChat ms={ms} />
    </div>
  )
}

/** One screenshot, `t` ms after Claude starts on it (negative: not yet). */
function SlideCard({ slide, t, editing }: { slide: Slide; t: number; editing: boolean }) {
  const painted = t >= 0
  const typed = Math.max(0, Math.min(slide.headline.length, Math.floor((t - 300) / 32)))
  const typing = typed > 0 && typed < slide.headline.length
  const subbed = t >= 1100
  const phone = t >= 1300
  return (
    <div className="relative">
      <div
        className={`@container relative aspect-[1320/2868] w-[clamp(92px,12.5vw,160px)] overflow-hidden rounded-[10px] text-center transition-shadow duration-300 ${
          painted ? "" : "outline-1 -outline-offset-1 outline-white/15 outline-dashed"
        } ${editing ? "shadow-[0_0_0_2px_#0a84ff]" : ""}`}
      >
        <div className="absolute inset-0 bg-[#232329]" />
        <div
          className="absolute inset-0 transition-opacity duration-700"
          style={{
            opacity: painted ? 1 : 0,
            background: `radial-gradient(120% 70% at 50% 0%, ${slide.glow}55, transparent 60%), linear-gradient(170deg, ${slide.from}, ${slide.to})`,
          }}
        />
        <div className="relative px-[7cqw] pt-[10cqw]">
          {painted ? (
            <div className="text-[10.5cqw] leading-[1.08] font-bold tracking-[-0.02em] text-white">
              {slide.headline.slice(0, typed)}
              {typing && <span className="ml-[0.5cqw] inline-block h-[9cqw] w-[0.9cqw] translate-y-[1cqw] bg-white" />}
            </div>
          ) : (
            <div className="flex flex-col items-center gap-[2.5cqw] pt-[1cqw]">
              <div className="h-[6cqw] w-[78%] rounded-full bg-white/10" />
              <div className="h-[6cqw] w-[52%] rounded-full bg-white/10" />
            </div>
          )}
          <div className="mt-[3cqw] text-[5.2cqw] text-white/80 transition-opacity duration-500" style={{ opacity: subbed ? 1 : 0 }}>
            {slide.sub}
          </div>
        </div>
        <div
          className="absolute inset-x-[10%] top-[37%] bottom-[-8%] rounded-[12cqw] bg-[#0b0b0f] p-[2.2cqw] shadow-[0_6cqw_12cqw_-4cqw_rgb(0_0_0/0.6)] transition-[transform,opacity] duration-700 ease-out"
          style={{
            transform: phone ? "translateY(0)" : "translateY(18%)",
            opacity: phone ? 1 : painted ? 0.35 : 0.2,
          }}
        >
          <div className="relative h-full overflow-hidden rounded-[10cqw] bg-[#0e0e16] px-[6cqw] pt-[11cqw]">
            <div className="absolute top-[3.2cqw] left-1/2 h-[5cqw] w-[24cqw] -translate-x-1/2 rounded-full bg-black" />
            {slide.screen === "sleep" && <SleepScreen />}
            {slide.screen === "mix" && <MixScreen />}
            {slide.screen === "alarm" && <AlarmScreen />}
          </div>
        </div>
      </div>
      {editing && (
        <span className="absolute -top-[22px] left-0 flex items-center gap-1 rounded-[5px] rounded-bl-none bg-[#d97757] px-1.5 py-0.5 text-[10px] font-semibold whitespace-nowrap text-white">
          Claude
        </span>
      )}
    </div>
  )
}

// The three Moonwake screens inside the phones, sized in container units so
// they scale with the slide.

function SleepScreen() {
  return (
    <div className="flex flex-col items-center text-white">
      <div className="self-start text-[4.6cqw] text-white/55">Tonight</div>
      <div className="relative mt-[5cqw] grid size-[44cqw] place-items-center rounded-full bg-[radial-gradient(circle_at_50%_40%,#4c3a9e,#1b1640_70%)] shadow-[0_0_10cqw_rgb(167_139_250/0.45)]">
        <div className="text-[11cqw] leading-none font-semibold tabular-nums">45:00</div>
      </div>
      <div className="mt-[4cqw] text-[4.6cqw] text-white/70">Rain on a tent</div>
      <div className="mt-[3.5cqw] flex gap-[2cqw] text-[3.8cqw]">
        <span className="rounded-full bg-[#7c3aed] px-[3cqw] py-[1cqw]">Rain</span>
        <span className="rounded-full bg-white/10 px-[3cqw] py-[1cqw]">Ocean</span>
        <span className="rounded-full bg-white/10 px-[3cqw] py-[1cqw]">Brown</span>
      </div>
      <div className="mt-[5cqw] grid size-[13cqw] place-items-center rounded-full bg-white">
        <span className="ml-[1cqw] border-y-[2.6cqw] border-l-[4cqw] border-y-transparent border-l-[#1b1640]" />
      </div>
    </div>
  )
}

function MixScreen() {
  const sounds = [
    { name: "Rain", level: 80, color: "#38bdf8" },
    { name: "Thunder", level: 35, color: "#a78bfa" },
    { name: "Campfire", level: 55, color: "#fb923c" },
    { name: "Wind", level: 65, color: "#5eead4" },
  ]
  return (
    <div className="text-left text-white">
      <div className="text-[6.4cqw] font-bold">Your mix</div>
      <div className="mt-[4cqw] flex flex-col gap-[4cqw]">
        {sounds.map((s) => (
          <div key={s.name}>
            <div className="flex items-center gap-[2cqw] text-[4.4cqw]">
              <span className="size-[3cqw] rounded-full" style={{ background: s.color }} />
              {s.name}
            </div>
            <div className="mt-[1.6cqw] h-[2.2cqw] rounded-full bg-white/10">
              <div className="h-full rounded-full" style={{ width: `${s.level}%`, background: s.color }} />
            </div>
          </div>
        ))}
      </div>
      <div className="mt-[5cqw] rounded-full bg-[#0891b2] py-[2.2cqw] text-center text-[4.4cqw] font-semibold">Save mix</div>
    </div>
  )
}

function AlarmScreen() {
  return (
    <div className="flex flex-col items-center text-white">
      <div className="self-start text-[4.6cqw] text-white/55">Alarm</div>
      <div className="mt-[3cqw] text-[17cqw] leading-none font-light tracking-[-0.03em] tabular-nums">6:45</div>
      <div className="relative mt-[4cqw] h-[16cqw] w-[48cqw] overflow-hidden">
        <div className="absolute inset-x-0 top-0 h-[32cqw] rounded-full bg-[radial-gradient(circle_at_50%_50%,#fde68a,#f97316_55%,transparent_72%)]" />
      </div>
      <div className="h-[0.6cqw] w-full bg-white/15" />
      <div className="mt-[4cqw] flex w-full items-center justify-between text-[4.2cqw]">
        <span>Sunrise fade · 30 min</span>
        <span className="flex h-[5.4cqw] w-[9.6cqw] items-center justify-end rounded-full bg-[#f97316] p-[0.6cqw]">
          <span className="size-[4.2cqw] rounded-full bg-white" />
        </span>
      </div>
      <div className="mt-[4cqw] flex w-full justify-between text-[3.8cqw] text-white/60">
        {["M", "T", "W", "T", "F"].map((d, i) => (
          <span key={i} className="grid size-[7cqw] place-items-center rounded-full bg-white/10 text-white">
            {d}
          </span>
        ))}
      </div>
    </div>
  )
}

/** Characters of `text` streamed `ms` after `at`, `perChar` ms apart. */
function streamed(text: string, ms: number, at: number, perChar: number) {
  return text.slice(0, Math.max(0, Math.floor((ms - at) / perChar)))
}

/** The design chat beside the canvas, played by the clock, with the reader's cursor. */
function DesignChat({ ms }: { ms: number }) {
  const aside = useRef<HTMLElement>(null)
  const spots = useRef<Partial<Record<Target, HTMLElement | null>>>({})
  const [points, setPoints] = useState<Partial<Record<Target, { x: number; y: number }>>>({})
  useEffect(() => {
    const measure = () => {
      const box = aside.current?.getBoundingClientRect()
      if (!box) return
      const next: Partial<Record<Target, { x: number; y: number }>> = {}
      for (const [name, node] of Object.entries(spots.current)) {
        const r = node?.getBoundingClientRect()
        if (r)
          next[name as Target] = {
            x: r.left - box.left + Math.min(r.width / 2, 40),
            y: r.top - box.top + r.height * 0.6,
          }
      }
      setPoints(next)
    }
    measure()
    window.addEventListener("resize", measure)
    return () => window.removeEventListener("resize", measure)
  }, [])

  const live = Number.isFinite(ms)
  const stop = [...DESIGN.cursor].reverse().find((c) => ms >= c.at)
  const clicking = DESIGN.cursor.some((c) => "click" in c && ms >= c.at + 450 && ms < c.at + 600)
  const cursorAt = stop && points[stop.to]
  const menu = ms >= DESIGN.folderMenu[0] && ms < DESIGN.folderMenu[1] ? "folder" : ms >= DESIGN.modelMenu[0] && ms < DESIGN.modelMenu[1] ? "model" : null
  const typed = ms < DESIGN.sendAt ? streamed(DESIGN.asked, ms, DESIGN.typeAt, 26) : ""
  const sent = ms >= DESIGN.sendAt
  const steps = DESIGN.steps.filter((s) => ms >= s.at)
  const working = ms >= DESIGN.introAt && ms < DESIGN.replyAt
  const set = (name: Target) => (node: HTMLElement | null) => {
    spots.current[name] = node
  }

  return (
    <aside ref={aside} aria-label="Design with AI" className="relative hidden w-[280px] shrink-0 flex-col border-l border-app-line bg-app-side lg:flex">
      <div className="flex items-center gap-2 border-b border-app-line px-4 py-3">
        <Icon name="sparkles" className="size-4 text-app-accent-soft" />
        <span className="text-[14px] font-semibold">Design with AI</span>
      </div>
      <div className="flex min-h-0 flex-1 flex-col justify-end gap-3 overflow-hidden p-4" aria-live="polite">
        {sent && <div className="ml-6 self-end rounded-2xl rounded-br-[4px] bg-app-accent px-3 py-2 text-white">{DESIGN.asked}</div>}
        {ms >= DESIGN.introAt && (
          <div className="flex flex-col gap-2">
            <div className="flex items-center gap-2 text-[12px] font-medium">
              <img src="/claude.svg" alt="" className="size-4" />
              Claude
              <span className="font-normal text-app-muted">Opus 5.5</span>
              {working && <span className="ml-auto size-1.5 animate-pulse rounded-full bg-[#d97757]" />}
            </div>
            <p className="leading-relaxed">{streamed(DESIGN.intro, ms, DESIGN.introAt, 16)}</p>
          </div>
        )}
        {steps.length > 0 && (
          <ul className="flex flex-col gap-1.5 text-[12px] text-app-muted">
            {steps.map((step) => (
              <li key={step.text} className="flex items-start gap-2">
                <Icon name="check" className="mt-0.5 size-3.5 shrink-0 text-app-up" />
                {step.text}
              </li>
            ))}
          </ul>
        )}
        {ms >= DESIGN.replyAt && <p className="leading-relaxed">{streamed(DESIGN.reply, ms, DESIGN.replyAt, 12)}</p>}
      </div>
      <Composer
        placeholder="Ask Claude to change a slide…"
        text={typed}
        caret={live && ms >= DESIGN.typeAt && ms < DESIGN.sendAt}
        menu={menu}
        hover={menu && stop ? stop.to : null}
        spot={set}
      />
      {live && cursorAt && ms < DESIGN.sendAt + 500 && (
        <svg
          viewBox="0 0 16 24"
          aria-hidden="true"
          className="pointer-events-none absolute top-0 left-0 z-20 h-[22px] w-[15px] drop-shadow-[0_1px_2px_rgb(0_0_0/0.5)] transition-transform duration-500 ease-in-out"
          style={{
            transform: `translate(${cursorAt.x}px, ${cursorAt.y}px) scale(${clicking ? 0.85 : 1})`,
          }}
        >
          <path d="M1 1v18l5-4.5 3.4 7.5 3-1.4-3.3-7.3H16L1 1Z" fill="#fff" stroke="#000" strokeWidth="1.3" strokeLinejoin="round" />
        </svg>
      )}
    </aside>
  )
}

const FOLDERS = ["moonwake", "moonwake-web"]
const MODELS = ["Opus 5.5", "Sonnet 5.5", "Haiku 5.5"]

/**
 * The message box under both AI panels: the project folder, the AI and its
 * model, as in the app. The design demo drives it (typed text, open menus,
 * the item under the cursor) and measures its parts with `spot`.
 */
function Composer({
  placeholder,
  text = "",
  caret = false,
  menu = null,
  hover = null,
  spot,
}: {
  placeholder: string
  text?: string
  caret?: boolean
  menu?: "folder" | "model" | null
  hover?: Target | null
  spot?: (name: Target) => (node: HTMLElement | null) => void
}) {
  const menuClass = (open: boolean) =>
    `absolute bottom-full left-0 z-10 mb-1.5 w-[150px] rounded-lg bg-[#2d2d33] p-1 text-[12px] shadow-[0_8px_24px_rgb(0_0_0/0.45)] ring-1 ring-white/10 transition-opacity duration-150 ${open ? "opacity-100" : "pointer-events-none opacity-0"}`
  const item = (active: boolean) => `flex items-center justify-between rounded-md px-2 py-1 ${active ? "bg-app-accent text-white" : ""}`
  return (
    <div className="border-t border-app-line p-3">
      <div className="rounded-xl bg-white/[0.05] p-2 ring-1 ring-app-line">
        <div ref={spot?.("input")} className="min-h-[36px] px-1 pb-2 leading-snug">
          {text || caret ? (
            <>
              {text}
              {caret && <span className="ml-px inline-block h-[13px] w-px translate-y-[2px] animate-pulse bg-app-text" />}
            </>
          ) : (
            <span className="text-app-muted">{placeholder}</span>
          )}
        </div>
        <div className="flex items-center gap-0.5 text-[11px] whitespace-nowrap text-app-muted">
          <span className="relative">
            <span
              ref={spot?.("folder")}
              className={`flex items-center gap-1 rounded px-1.5 py-0.5 ${menu === "folder" ? "bg-white/[0.08] text-app-text" : ""}`}
            >
              <Icon name="folder" className="size-3" /> moonwake
            </span>
            <div className={menuClass(menu === "folder")} aria-hidden={menu !== "folder"}>
              <div className="px-2 pt-1 pb-1.5 text-[10px] text-app-muted">Project</div>
              {FOLDERS.map((name, i) => (
                <div key={name} ref={i === 0 ? spot?.("folderItem") : undefined} className={item(menu === "folder" && hover === "folderItem" && i === 0)}>
                  {name}
                  {i === 0 && <Icon name="check" className="size-3" />}
                </div>
              ))}
              <div className="my-1 h-px bg-white/10" />
              <div className="px-2 py-1">Choose folder…</div>
            </div>
          </span>
          <span className="flex items-center gap-1 rounded px-1.5 py-0.5" title="Claude Code">
            <img src="/claude.svg" alt="" className="size-3" /> Claude
          </span>
          <span className="relative">
            <span ref={spot?.("model")} className={`block rounded px-1.5 py-0.5 ${menu === "model" ? "bg-white/[0.08] text-app-text" : ""}`}>
              Opus 5.5
            </span>
            <div className={menuClass(menu === "model")} aria-hidden={menu !== "model"}>
              <div className="px-2 pt-1 pb-1.5 text-[10px] text-app-muted">Model</div>
              {MODELS.map((name, i) => (
                <div key={name} ref={i === 0 ? spot?.("modelItem") : undefined} className={item(menu === "model" && hover === "modelItem" && i === 0)}>
                  {name}
                  {i === 0 && <Icon name="check" className="size-3" />}
                </div>
              ))}
            </div>
          </span>
          <span ref={spot?.("send")} className="ml-auto grid size-5 place-items-center rounded-full bg-app-accent text-white">
            ↑
          </span>
        </div>
      </div>
    </div>
  )
}

/* ------------------------------------------------------------------------ */
/* AI panel                                                                  */
/* ------------------------------------------------------------------------ */

function AiPanel({ play, onClose }: { play: boolean; onClose: () => void }) {
  // The conversation plays out once when the reader opens the panel.
  const [shown, setShown] = useState(play ? 0 : DEMO_CHAT.steps.length + 1)
  useEffect(() => {
    if (!play) return
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setShown(DEMO_CHAT.steps.length + 1)
      return
    }
    const timers = Array.from({ length: DEMO_CHAT.steps.length + 1 }, (_, i) => setTimeout(() => setShown(i + 1), 450 * (i + 1)))
    return () => timers.forEach(clearTimeout)
  }, [play])
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
        <div className="flex items-center gap-2 text-[12px] font-medium">
          <img src="/claude.svg" alt="" className="size-4" />
          Claude
          <span className="font-normal text-app-muted">Opus 5.5</span>
        </div>
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
      <Composer placeholder="Ask about your listing, keywords…" />
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
