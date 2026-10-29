import { keepPreviousData, useQuery } from "@tanstack/react-query"
import { Link, createFileRoute } from "@tanstack/react-router"
import { useState } from "react"
import { ErrorNote, PageBody, PageHeader, StatusBadge } from "@/components/page"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs"
import { adminApi, call } from "@/lib/api"
import { date } from "@/lib/format"

export const Route = createFileRoute("/_admin/subscribers")({ component: Subscribers })

const filters = [
  { value: "entitled", label: "Current" },
  { value: "past_due", label: "Past due" },
  { value: "trialing", label: "Trialing" },
  { value: "canceled", label: "Canceled" },
  { value: "all", label: "All" },
] as const
type Filter = (typeof filters)[number]["value"]
const PAGE = 50

function Subscribers() {
  const [status, setStatus] = useState<Filter>("entitled")
  const [q, setQ] = useState("")
  const [offset, setOffset] = useState(0)

  const list = useQuery({
    queryKey: ["subscriptions", status, q, offset],
    queryFn: () =>
      call(adminApi.subscriptions.$get({ query: { status, q: q || undefined, limit: String(PAGE), offset: String(offset) } })),
    placeholderData: keepPreviousData,
  })

  return (
    <>
      <PageHeader title="Subscribers" />
      <PageBody>
        <div className="flex flex-wrap items-center gap-3">
          <Tabs
            value={status}
            onValueChange={(v) => {
              setStatus(v as Filter)
              setOffset(0)
            }}
          >
            <TabsList>
              {filters.map((f) => (
                <TabsTrigger key={f.value} value={f.value}>
                  {f.label}
                </TabsTrigger>
              ))}
            </TabsList>
          </Tabs>
          <Input
            placeholder="Search email or name"
            className="max-w-xs"
            value={q}
            onChange={(e) => {
              setQ(e.target.value)
              setOffset(0)
            }}
          />
          <span className="ml-auto text-sm text-muted-foreground">{list.data?.total ?? 0} total</span>
        </div>
        <ErrorNote error={list.error} />
        <div className="rounded-lg border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Customer</TableHead>
                <TableHead>Plan</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Billing</TableHead>
                <TableHead>Renews / ends</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {list.data?.rows.map((s) => (
                <TableRow key={s.id}>
                  <TableCell>
                    <Link to="/users/$userId" params={{ userId: s.userId }} className="font-medium hover:underline">
                      {s.email}
                    </Link>
                    {s.name && <div className="text-xs text-muted-foreground">{s.name}</div>}
                  </TableCell>
                  <TableCell>{s.planName ?? s.plan}</TableCell>
                  <TableCell>
                    <StatusBadge status={s.status} />
                    {s.cancelAtPeriodEnd && <span className="ml-2 text-xs text-muted-foreground">cancels at period end</span>}
                  </TableCell>
                  <TableCell className="capitalize">{s.interval ? `${s.interval}ly` : "—"}</TableCell>
                  <TableCell>{s.status === "trialing" ? `Trial ends ${date(s.trialEnd)}` : date(s.periodEnd)}</TableCell>
                </TableRow>
              ))}
              {list.data?.rows.length === 0 && (
                <TableRow>
                  <TableCell colSpan={5} className="h-24 text-center text-muted-foreground">
                    No subscriptions match.
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
        <Pager offset={offset} total={list.data?.total ?? 0} onChange={setOffset} />
      </PageBody>
    </>
  )
}

export function Pager({ offset, total, onChange }: { offset: number; total: number; onChange: (n: number) => void }) {
  if (total <= PAGE) return null
  return (
    <div className="flex items-center justify-end gap-2 text-sm">
      <span className="text-muted-foreground">
        {offset + 1}–{Math.min(offset + PAGE, total)} of {total}
      </span>
      <Button variant="outline" size="sm" disabled={offset === 0} onClick={() => onChange(Math.max(0, offset - PAGE))}>
        Previous
      </Button>
      <Button variant="outline" size="sm" disabled={offset + PAGE >= total} onClick={() => onChange(offset + PAGE)}>
        Next
      </Button>
    </div>
  )
}
