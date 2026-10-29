import { Badge } from "@/components/ui/badge"
import { Separator } from "@/components/ui/separator"
import { SidebarTrigger } from "@/components/ui/sidebar"

export function PageHeader({ title, actions }: { title: string; actions?: React.ReactNode }) {
  return (
    <header className="flex h-14 shrink-0 items-center gap-2 border-b px-4">
      <SidebarTrigger className="-ml-1" />
      <Separator orientation="vertical" className="mr-2 data-[orientation=vertical]:h-4" />
      <h1 className="text-sm font-medium">{title}</h1>
      <div className="ml-auto flex items-center gap-2">{actions}</div>
    </header>
  )
}

export function PageBody({ children }: { children: React.ReactNode }) {
  return <div className="flex flex-1 flex-col gap-4 p-4 md:p-6">{children}</div>
}

const statusVariant: Record<string, "default" | "secondary" | "destructive" | "outline"> = {
  active: "default",
  trialing: "secondary",
  past_due: "destructive",
  unpaid: "destructive",
  canceled: "outline",
  incomplete: "outline",
  incomplete_expired: "outline",
  paused: "outline",
}

export function StatusBadge({ status }: { status: string | null }) {
  return <Badge variant={statusVariant[status ?? ""] ?? "outline"}>{(status ?? "unknown").replaceAll("_", " ")}</Badge>
}

export function ErrorNote({ error }: { error: Error | null }) {
  if (!error) return null
  return <p className="rounded-md border border-destructive/30 bg-destructive/5 p-3 text-sm text-destructive">{error.message}</p>
}
