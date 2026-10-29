import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { createFileRoute } from "@tanstack/react-router"
import { ErrorNote, PageBody, PageHeader } from "@/components/page"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { adminApi, call } from "@/lib/api"
import { date, money, stripeDashboardUrl } from "@/lib/format"

export const Route = createFileRoute("/_admin/alerts")({ component: Alerts })

const kindLabel = { refund: "Refund", dispute: "Dispute", fraud_warning: "Fraud warning" } as const

function Alerts() {
  const queryClient = useQueryClient()
  const alerts = useQuery({ queryKey: ["alerts"], queryFn: () => call(adminApi.alerts.$get()) })
  const resolve = useMutation({
    mutationFn: (id: string) => call(adminApi.alerts[":id"].resolve.$post({ param: { id } })),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["alerts"] })
      queryClient.invalidateQueries({ queryKey: ["stats"] })
    },
  })

  return (
    <>
      <PageHeader title="Alerts" />
      <PageBody>
        <p className="text-sm text-muted-foreground">
          Money-at-risk events from Stripe. Disputes have a response deadline: answer them in the Stripe Dashboard.
        </p>
        <ErrorNote error={alerts.error} />
        <div className="rounded-lg border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Kind</TableHead>
                <TableHead>Details</TableHead>
                <TableHead>Amount</TableHead>
                <TableHead>When</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {alerts.data?.map((a) => (
                <TableRow key={a.id} className={a.resolvedAt ? "opacity-50" : undefined}>
                  <TableCell>
                    <Badge variant={a.kind === "refund" ? "secondary" : "destructive"}>{kindLabel[a.kind]}</Badge>
                  </TableCell>
                  <TableCell className="max-w-md">
                    <div>{a.message}</div>
                    {a.stripeCustomerId && (
                      <a
                        className="text-xs text-muted-foreground underline-offset-4 hover:underline"
                        href={stripeDashboardUrl(`customers/${a.stripeCustomerId}`)}
                        target="_blank"
                        rel="noreferrer"
                      >
                        {a.stripeCustomerId}
                      </a>
                    )}
                  </TableCell>
                  <TableCell>{money(a.amount, a.currency ?? undefined)}</TableCell>
                  <TableCell>{date(a.createdAt)}</TableCell>
                  <TableCell className="text-right">
                    {!a.resolvedAt && (
                      <Button size="sm" variant="outline" onClick={() => resolve.mutate(a.id)}>
                        Mark resolved
                      </Button>
                    )}
                  </TableCell>
                </TableRow>
              ))}
              {alerts.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={5} className="h-24 text-center text-muted-foreground">
                    No alerts. Refunds, disputes and early fraud warnings will show up here.
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
      </PageBody>
    </>
  )
}
