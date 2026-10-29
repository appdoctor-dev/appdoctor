import { useQuery } from "@tanstack/react-query"
import { Link, createFileRoute } from "@tanstack/react-router"
import { ErrorNote, PageBody, PageHeader } from "@/components/page"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { adminApi, call } from "@/lib/api"
import { money } from "@/lib/format"

export const Route = createFileRoute("/_admin/")({ component: Dashboard })

function Dashboard() {
  const stats = useQuery({ queryKey: ["stats"], queryFn: () => call(adminApi.stats.$get()) })
  const s = stats.data

  const tiles = s
    ? [
        { label: "Est. MRR", value: money(s.estimatedMrr, s.currency), hint: "At current plan prices" },
        { label: "Active", value: s.active, hint: "Paying subscribers" },
        { label: "Trialing", value: s.trialing, hint: "Not yet billed" },
        { label: "Past due", value: s.pastDue, hint: "Payment failing, in grace period" },
        { label: "Canceled (30d)", value: s.canceledLast30d },
        { label: "Users", value: s.users, hint: `${s.signupsLast30d} new in 30 days` },
      ]
    : []

  return (
    <>
      <PageHeader title="Dashboard" />
      <PageBody>
        <ErrorNote error={stats.error} />
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {stats.isPending
            ? Array.from({ length: 6 }, (_, i) => <Skeleton key={i} className="h-28" />)
            : tiles.map((t) => (
                <Card key={t.label}>
                  <CardHeader>
                    <CardDescription>{t.label}</CardDescription>
                    <CardTitle className="text-3xl tabular-nums">{t.value}</CardTitle>
                  </CardHeader>
                  {t.hint && <CardContent className="text-xs text-muted-foreground">{t.hint}</CardContent>}
                </Card>
              ))}
        </div>

        {s && (
          <div className="grid gap-4 lg:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle>Subscribers by plan</CardTitle>
              </CardHeader>
              <CardContent className="flex flex-col gap-2 text-sm">
                {s.byPlan.length === 0 && <p className="text-muted-foreground">No subscribers yet.</p>}
                {s.byPlan.map((p) => (
                  <div key={p.plan} className="flex justify-between">
                    <span>{p.name}</span>
                    <span className="tabular-nums">{p.count}</span>
                  </div>
                ))}
              </CardContent>
            </Card>
            <Card>
              <CardHeader>
                <CardTitle>Needs attention</CardTitle>
                <CardDescription>Refunds, disputes and fraud warnings from Stripe</CardDescription>
              </CardHeader>
              <CardContent className="text-sm">
                {s.openAlerts === 0 ? (
                  <p className="text-muted-foreground">Nothing open.</p>
                ) : (
                  <Link to="/alerts" className="font-medium text-destructive underline-offset-4 hover:underline">
                    {s.openAlerts} open alert{s.openAlerts === 1 ? "" : "s"}
                  </Link>
                )}
              </CardContent>
            </Card>
          </div>
        )}
      </PageBody>
    </>
  )
}
