import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { createFileRoute } from "@tanstack/react-router"
import { ExternalLinkIcon } from "lucide-react"
import { toast } from "sonner"
import { ErrorNote, PageBody, PageHeader, StatusBadge } from "@/components/page"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { adminApi, call } from "@/lib/api"
import { date, money, relative, stripeDashboardUrl } from "@/lib/format"

export const Route = createFileRoute("/_admin/users/$userId")({ component: UserDetail })

function UserDetail() {
  const { userId } = Route.useParams()
  const queryClient = useQueryClient()
  const detail = useQuery({
    queryKey: ["user", userId],
    queryFn: () => call(adminApi.users[":id"].$get({ param: { id: userId } })),
  })
  const removeDevice = useMutation({
    mutationFn: (deviceId: string) =>
      call(adminApi.users[":id"].devices[":deviceId"].$delete({ param: { id: userId, deviceId } })),
    onSuccess: () => {
      toast.success("Device signed out")
      queryClient.invalidateQueries({ queryKey: ["user", userId] })
    },
    onError: (e) => toast.error(e.message),
  })

  const d = detail.data
  return (
    <>
      <PageHeader
        title={d?.user.email ?? "User"}
        actions={
          d?.user.stripeCustomerId && (
            <Button variant="outline" size="sm" asChild>
              <a href={stripeDashboardUrl(`customers/${d.user.stripeCustomerId}`)} target="_blank" rel="noreferrer">
                Open in Stripe <ExternalLinkIcon />
              </a>
            </Button>
          )
        }
      />
      <PageBody>
        <ErrorNote error={detail.error} />
        {d && (
          <>
            <Card>
              <CardHeader>
                <CardTitle>Subscriptions</CardTitle>
              </CardHeader>
              <CardContent>
                {d.subscriptions.length === 0 ? (
                  <p className="text-sm text-muted-foreground">Never subscribed.</p>
                ) : (
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Plan</TableHead>
                        <TableHead>Status</TableHead>
                        <TableHead>Billing</TableHead>
                        <TableHead>Period ends</TableHead>
                        <TableHead />
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {d.subscriptions.map((s) => (
                        <TableRow key={s.id}>
                          <TableCell>{s.plan}</TableCell>
                          <TableCell>
                            <StatusBadge status={s.status} />
                            {s.cancelAtPeriodEnd && <span className="ml-2 text-xs text-muted-foreground">canceling</span>}
                          </TableCell>
                          <TableCell className="capitalize">{s.billingInterval ? `${s.billingInterval}ly` : "—"}</TableCell>
                          <TableCell>{date(s.periodEnd)}</TableCell>
                          <TableCell className="text-right">
                            {s.stripeSubscriptionId && (
                              <a
                                className="text-xs underline-offset-4 hover:underline"
                                href={stripeDashboardUrl(`subscriptions/${s.stripeSubscriptionId}`)}
                                target="_blank"
                                rel="noreferrer"
                              >
                                Stripe
                              </a>
                            )}
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                )}
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle>Devices</CardTitle>
              </CardHeader>
              <CardContent>
                {d.devices.length === 0 ? (
                  <p className="text-sm text-muted-foreground">Not signed in on any device.</p>
                ) : (
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Device</TableHead>
                        <TableHead>Platform</TableHead>
                        <TableHead>Version</TableHead>
                        <TableHead>Last seen</TableHead>
                        <TableHead />
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {d.devices.map((dev) => (
                        <TableRow key={dev.id}>
                          <TableCell>{dev.deviceName || dev.deviceId.slice(0, 8)}</TableCell>
                          <TableCell>{dev.platform}</TableCell>
                          <TableCell>{dev.appVersion || "—"}</TableCell>
                          <TableCell>{relative(dev.lastSeenAt)}</TableCell>
                          <TableCell className="text-right">
                            <Button
                              variant="ghost"
                              size="sm"
                              disabled={removeDevice.isPending}
                              onClick={() => removeDevice.mutate(dev.deviceId)}
                            >
                              Sign out
                            </Button>
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                )}
              </CardContent>
            </Card>

            {d.alerts.length > 0 && (
              <Card>
                <CardHeader>
                  <CardTitle>Alerts</CardTitle>
                </CardHeader>
                <CardContent className="flex flex-col gap-2 text-sm">
                  {d.alerts.map((a) => (
                    <div key={a.id} className="flex justify-between gap-4">
                      <span>{a.message}</span>
                      <span className="shrink-0 text-muted-foreground">
                        {money(a.amount, a.currency ?? undefined)} · {date(a.createdAt)}
                      </span>
                    </div>
                  ))}
                </CardContent>
              </Card>
            )}
          </>
        )}
      </PageBody>
    </>
  )
}
