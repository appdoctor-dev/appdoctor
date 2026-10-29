import { keepPreviousData, useQuery } from "@tanstack/react-query"
import { Link, createFileRoute } from "@tanstack/react-router"
import { useState } from "react"
import { ErrorNote, PageBody, PageHeader } from "@/components/page"
import { Badge } from "@/components/ui/badge"
import { Input } from "@/components/ui/input"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { adminApi, call } from "@/lib/api"
import { date } from "@/lib/format"
import { Pager } from "./subscribers"

export const Route = createFileRoute("/_admin/users/")({ component: Users })

function Users() {
  const [q, setQ] = useState("")
  const [offset, setOffset] = useState(0)
  const list = useQuery({
    queryKey: ["users", q, offset],
    queryFn: () => call(adminApi.users.$get({ query: { q: q || undefined, limit: "50", offset: String(offset) } })),
    placeholderData: keepPreviousData,
  })

  return (
    <>
      <PageHeader title="Users" />
      <PageBody>
        <div className="flex items-center gap-3">
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
                <TableHead>Email</TableHead>
                <TableHead>Role</TableHead>
                <TableHead>Stripe customer</TableHead>
                <TableHead>Joined</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {list.data?.rows.map((u) => (
                <TableRow key={u.id}>
                  <TableCell>
                    <Link to="/users/$userId" params={{ userId: u.id }} className="font-medium hover:underline">
                      {u.email}
                    </Link>
                    {u.banned && (
                      <Badge variant="destructive" className="ml-2">
                        banned
                      </Badge>
                    )}
                  </TableCell>
                  <TableCell>{u.role === "admin" ? <Badge>admin</Badge> : "user"}</TableCell>
                  <TableCell className="font-mono text-xs">{u.stripeCustomerId ?? "—"}</TableCell>
                  <TableCell>{date(u.createdAt)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
        <Pager offset={offset} total={list.data?.total ?? 0} onChange={setOffset} />
      </PageBody>
    </>
  )
}
