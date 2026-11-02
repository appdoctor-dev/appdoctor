import { useQuery } from "@tanstack/react-query"
import { createFileRoute } from "@tanstack/react-router"
import { useState } from "react"
import { ErrorNote, PageBody, PageHeader } from "@/components/page"
import { Badge } from "@/components/ui/badge"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { adminApi, call } from "@/lib/api"

export const Route = createFileRoute("/_admin/analytics")({ component: Analytics })

const PERIODS = ["7", "30", "90", "365"]

const CHANNELS: Record<string, string> = {
  search: "Search",
  ai: "AI",
  social: "Social",
  referral: "Link",
  email: "Email",
  campaign: "Campaign",
  direct: "Direct",
}

const AGENT_KINDS: Record<string, string> = {
  assistant: "Answering someone",
  "ai-search": "AI search index",
  "ai-training": "AI training",
  search: "Search engine",
}

const regions = new Intl.DisplayNames(undefined, { type: "region" })

function country(code: string | null) {
  if (!code || !/^[A-Z]{2}$/.test(code) || code === "XX" || code === "T1") return "Unknown"
  const flag = String.fromCodePoint(...[...code].map((c) => 0x1f1a5 + c.charCodeAt(0)))
  let name = code
  try {
    name = regions.of(code) ?? code
  } catch {}
  return `${flag} ${name}`
}

const time = (value: string) =>
  new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(new Date(value))
const percent = (n: number) => `${(n * 100).toFixed(1)}%`

function Analytics() {
  const [days, setDays] = useState("30")
  const stats = useQuery({
    queryKey: ["analytics", days],
    queryFn: () => call(adminApi.analytics.$get({ query: { days } })),
  })
  const search = useQuery({
    queryKey: ["analytics-search", days],
    queryFn: () => call(adminApi.analytics.search.$get({ query: { days } })),
  })
  const s = stats.data

  const tiles = s
    ? [
        { label: "Visits", value: s.totals.visits, hint: "People arriving from outside the site" },
        { label: "Countries", value: s.totals.countries },
        { label: "From search", value: s.totals.fromSearch, hint: "Google, Bing, DuckDuckGo…" },
        { label: "From AI", value: s.totals.fromAi, hint: "ChatGPT, Perplexity, Claude, Gemini…" },
        { label: "Agent reads", value: s.totals.agentReads, hint: "Pages read by AI and search bots" },
      ]
    : []

  return (
    <>
      <PageHeader
        title="Analytics"
        actions={
          <Select value={days} onValueChange={setDays}>
            <SelectTrigger size="sm" className="w-36">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {PERIODS.map((d) => (
                <SelectItem key={d} value={d}>
                  Last {d} days
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        }
      />
      <PageBody>
        <p className="max-w-3xl text-sm text-muted-foreground">
          Visits to appdoctor.so, counted without cookies: each arrival from another site, a search, an AI answer or a
          typed address. Search engines no longer say what someone searched for, so search terms come from Google
          Search Console.
        </p>
        <ErrorNote error={stats.error} />
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-5">
          {stats.isPending
            ? Array.from({ length: 5 }, (_, i) => <Skeleton key={i} className="h-28" />)
            : tiles.map((t) => (
                <Card key={t.label}>
                  <CardHeader>
                    <CardDescription>{t.label}</CardDescription>
                    <CardTitle className="text-3xl tabular-nums">{t.value.toLocaleString()}</CardTitle>
                    {t.hint && <CardDescription className="text-xs">{t.hint}</CardDescription>}
                  </CardHeader>
                </Card>
              ))}
        </div>

        {s && (
          <>
            <DailyChart days={Number(days)} rows={s.byDay} />

            <div className="grid gap-4 xl:grid-cols-2">
              <Breakdown
                title="Where they came from"
                empty="No visits yet."
                rows={s.sources.map((r) => ({
                  key: `${r.source}-${r.channel}`,
                  label: r.source ?? "Unknown",
                  tag: r.channel ? CHANNELS[r.channel] : undefined,
                  value: r.visits,
                }))}
              />
              <Breakdown
                title="Countries"
                empty="No visits yet."
                rows={s.countries.map((r) => ({ key: r.country ?? "?", label: country(r.country), value: r.visits }))}
              />
              <Breakdown
                title="Landing pages"
                description="The first page people saw"
                empty="No visits yet."
                rows={s.pages.map((r) => ({ key: r.path, label: r.path, mono: true, value: r.visits }))}
              />
              <div className="grid gap-4">
                <Breakdown
                  title="Channels"
                  empty="No visits yet."
                  rows={s.channels.map((r) => ({
                    key: r.channel ?? "?",
                    label: (r.channel && CHANNELS[r.channel]) ?? "Unknown",
                    value: r.visits,
                  }))}
                />
                <Breakdown
                  title="Devices"
                  empty="No visits yet."
                  rows={s.devices.map((r) => ({
                    key: r.device ?? "?",
                    label: r.device ? r.device[0]!.toUpperCase() + r.device.slice(1) : "Unknown",
                    value: r.visits,
                  }))}
                />
              </div>
            </div>
          </>
        )}

        <SearchQueries data={search.data} error={search.error} pending={search.isPending} />

        {s && (
          <>
            {s.queries.length > 0 && (
              <Breakdown
                title="Search terms carried by links"
                description="From the few engines that still pass them, and utm_term in campaign links"
                empty=""
                rows={s.queries.map((r) => ({
                  key: `${r.query}-${r.source}`,
                  label: r.query ?? "",
                  tag: r.source ?? undefined,
                  value: r.visits,
                }))}
              />
            )}

            <div className="grid gap-4 xl:grid-cols-2">
              <Breakdown
                title="AI and search agents"
                description="Bots reading the site, by the name they give. “Answering someone” means an AI fetched a page while answering a person’s question."
                empty="No agents yet."
                rows={s.agents.map((r) => ({
                  key: `${r.agent}-${r.kind}`,
                  label: r.agent ?? "Unknown",
                  tag: r.kind ? AGENT_KINDS[r.kind] : undefined,
                  value: r.reads,
                }))}
              />
              <Breakdown
                title="Pages AI read"
                description="What AI assistants and AI search fetched, a hint at what people ask them"
                empty="Nothing yet."
                rows={s.agentPages.map((r) => ({ key: r.path, label: r.path, mono: true, value: r.reads }))}
              />
            </div>

            {s.campaigns.length > 0 && (
              <Card>
                <CardHeader>
                  <CardTitle>Campaigns</CardTitle>
                  <CardDescription>Links tagged with utm_source, utm_medium or utm_campaign</CardDescription>
                </CardHeader>
                <CardContent>
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Source</TableHead>
                        <TableHead>Medium</TableHead>
                        <TableHead>Campaign</TableHead>
                        <TableHead className="text-right">Visits</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {s.campaigns.map((c) => (
                        <TableRow key={`${c.source}-${c.medium}-${c.campaign}`}>
                          <TableCell>{c.source ?? "—"}</TableCell>
                          <TableCell>{c.medium ?? "—"}</TableCell>
                          <TableCell>{c.campaign ?? "—"}</TableCell>
                          <TableCell className="text-right tabular-nums">{c.visits}</TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </CardContent>
              </Card>
            )}

            <Card>
              <CardHeader>
                <CardTitle>Latest visits</CardTitle>
                <CardDescription>Each arrival: when, from where, and the page it landed on</CardDescription>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>When</TableHead>
                      <TableHead>Country</TableHead>
                      <TableHead>Source</TableHead>
                      <TableHead>Landed on</TableHead>
                      <TableHead>Search terms</TableHead>
                      <TableHead>Device</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {s.recent.map((r, i) => (
                      <TableRow key={i}>
                        <TableCell className="whitespace-nowrap text-muted-foreground">{time(r.at)}</TableCell>
                        <TableCell className="whitespace-nowrap">{country(r.country)}</TableCell>
                        <TableCell>
                          {r.source}
                          {r.channel && (
                            <Badge variant="secondary" className="ml-2">
                              {CHANNELS[r.channel]}
                            </Badge>
                          )}
                        </TableCell>
                        <TableCell className="font-mono text-xs">{r.path}</TableCell>
                        <TableCell>{r.query ?? <span className="text-muted-foreground">—</span>}</TableCell>
                        <TableCell className="capitalize text-muted-foreground">{r.device}</TableCell>
                      </TableRow>
                    ))}
                    {s.recent.length === 0 && (
                      <TableRow>
                        <TableCell colSpan={6} className="py-8 text-center text-muted-foreground">
                          No visits in this period.
                        </TableCell>
                      </TableRow>
                    )}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>

            {s.recentAgents.length > 0 && (
              <Card>
                <CardHeader>
                  <CardTitle>Latest agent reads</CardTitle>
                </CardHeader>
                <CardContent>
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>When</TableHead>
                        <TableHead>Agent</TableHead>
                        <TableHead>Page</TableHead>
                        <TableHead>From</TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {s.recentAgents.map((r, i) => (
                        <TableRow key={i}>
                          <TableCell className="whitespace-nowrap text-muted-foreground">{time(r.at)}</TableCell>
                          <TableCell>
                            {r.agent}
                            {r.kind && (
                              <Badge variant="secondary" className="ml-2">
                                {AGENT_KINDS[r.kind]}
                              </Badge>
                            )}
                          </TableCell>
                          <TableCell className="font-mono text-xs">{r.path}</TableCell>
                          <TableCell className="whitespace-nowrap">{country(r.country)}</TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </CardContent>
              </Card>
            )}
          </>
        )}
      </PageBody>
    </>
  )
}

type Row = { key: string; label: string; value: number; tag?: string; mono?: boolean }

/** A ranked list with a bar for each row's share of the total. */
function Breakdown({ title, description, rows, empty }: { title: string; description?: string; rows: Row[]; empty: string }) {
  const total = rows.reduce((n, r) => n + r.value, 0)
  return (
    <Card>
      <CardHeader>
        <CardTitle>{title}</CardTitle>
        {description && <CardDescription>{description}</CardDescription>}
      </CardHeader>
      <CardContent className="flex flex-col gap-1.5 text-sm">
        {rows.length === 0 && <p className="text-muted-foreground">{empty}</p>}
        {rows.map((r) => (
          <div key={r.key} className="relative flex items-center gap-2 rounded-md px-2 py-1">
            <div
              className="absolute inset-y-0 left-0 rounded-md bg-primary/10"
              style={{ width: `${total ? (r.value / total) * 100 : 0}%` }}
            />
            <span className={`relative min-w-0 flex-1 truncate ${r.mono ? "font-mono text-xs" : ""}`}>{r.label}</span>
            {r.tag && (
              <Badge variant="outline" className="relative">
                {r.tag}
              </Badge>
            )}
            <span className="relative w-12 text-right tabular-nums">{r.value.toLocaleString()}</span>
          </div>
        ))}
      </CardContent>
    </Card>
  )
}

/** Visits per day as bars, with agent reads beside them; days with nothing still get a slot. */
function DailyChart({ days, rows }: { days: number; rows: { day: string; people: number; agents: number }[] }) {
  const byDay = new Map(rows.map((r) => [r.day, r]))
  const slots = Array.from({ length: days }, (_, i) => {
    const day = new Date(Date.now() - (days - 1 - i) * 86_400_000).toISOString().slice(0, 10)
    return { day, people: byDay.get(day)?.people ?? 0, agents: byDay.get(day)?.agents ?? 0 }
  })
  const max = Math.max(1, ...slots.map((s) => s.people))
  return (
    <Card>
      <CardHeader>
        <CardTitle>Visits per day</CardTitle>
        <CardDescription>Days in UTC. Hover a bar for the numbers.</CardDescription>
      </CardHeader>
      <CardContent>
        <div className="flex h-40 items-end gap-px">
          {slots.map((s) => (
            <div
              key={s.day}
              className="group flex h-full flex-1 flex-col justify-end"
              title={`${s.day}: ${s.people} visits, ${s.agents} agent reads`}
            >
              <div
                className="min-h-px rounded-t-sm bg-primary/70 group-hover:bg-primary"
                style={{ height: `${(s.people / max) * 100}%` }}
              />
            </div>
          ))}
        </div>
        <div className="mt-2 flex justify-between text-xs text-muted-foreground">
          <span>{slots[0]?.day}</span>
          <span>{slots.at(-1)?.day}</span>
        </div>
      </CardContent>
    </Card>
  )
}

type SearchData = Awaited<ReturnType<typeof fetchSearch>>
const fetchSearch = (days: string) => call(adminApi.analytics.search.$get({ query: { days } }))

function SearchQueries({ data, error, pending }: { data?: SearchData; error: Error | null; pending: boolean }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Google searches</CardTitle>
        <CardDescription>
          What people searched on Google when appdoctor.so showed up, from Search Console. Its numbers arrive two or
          three days late.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4 text-sm">
        <ErrorNote error={error} />
        {pending && <Skeleton className="h-24" />}
        {data && !data.configured && (
          <div className="flex max-w-3xl flex-col gap-2 text-muted-foreground">
            <p>Not connected yet. To see the searches:</p>
            <ol className="list-decimal space-y-1 pl-5">
              <li>Add appdoctor.so to Google Search Console as a Domain property.</li>
              <li>
                In Google Cloud, turn on the Google Search Console API, create a service account and download a JSON key
                for it.
              </li>
              <li>In Search Console's settings, add the service account's email as a user.</li>
              <li>
                Run <code className="rounded bg-muted px-1">apps/api/scripts/search-console.sh &lt;key.json&gt;</code>.
              </li>
            </ol>
          </div>
        )}
        {data?.configured && "error" in data && (
          <p className="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-destructive">
            Search Console said: {data.error}. Check that {data.account} is a user of {data.site}.
          </p>
        )}
        {data?.configured && "queries" in data && (
          <>
            {data.queries.length === 0 && <p className="text-muted-foreground">No searches in this period yet.</p>}
            {data.queries.length > 0 && (
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Search</TableHead>
                    <TableHead>Landed on</TableHead>
                    <TableHead className="text-right">Clicks</TableHead>
                    <TableHead className="text-right">Shown</TableHead>
                    <TableHead className="text-right">Click rate</TableHead>
                    <TableHead className="text-right">Position</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {data.pages.map((r) => (
                    <TableRow key={`${r.query}-${r.page}`}>
                      <TableCell>{r.query}</TableCell>
                      <TableCell className="font-mono text-xs">{r.page}</TableCell>
                      <TableCell className="text-right tabular-nums">{r.clicks}</TableCell>
                      <TableCell className="text-right tabular-nums">{r.impressions}</TableCell>
                      <TableCell className="text-right tabular-nums">{percent(r.ctr)}</TableCell>
                      <TableCell className="text-right tabular-nums">{r.position.toFixed(1)}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            )}
          </>
        )}
      </CardContent>
    </Card>
  )
}
