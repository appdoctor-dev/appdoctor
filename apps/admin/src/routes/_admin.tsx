import { useQuery } from "@tanstack/react-query"
import { Outlet, createFileRoute } from "@tanstack/react-router"
import { Loader2Icon } from "lucide-react"
import { AppSidebar } from "@/components/app-sidebar"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"

// Cloudflare Access decides who reaches this app; this only reads who that is.
export const Route = createFileRoute("/_admin")({ ssr: false, component: AdminLayout })

function AdminLayout() {
  const identity = useQuery({
    queryKey: ["whoami"],
    queryFn: async () => {
      const res = await fetch("/api/whoami")
      if (!res.ok) {
        const body = (await res.json().catch(() => null)) as { message?: string } | null
        throw new Error(body?.message ?? "Not authorized")
      }
      return (await res.json()) as { email: string }
    },
    staleTime: Infinity,
  })

  if (identity.isPending) {
    return (
      <div className="flex min-h-svh items-center justify-center">
        <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
      </div>
    )
  }
  if (identity.error) {
    return (
      <main className="flex min-h-svh flex-col items-center justify-center gap-2 p-4 text-center">
        <p className="font-medium">{identity.error.message}</p>
        <p className="max-w-sm text-sm text-muted-foreground">
          The admin is protected by Cloudflare Access. Locally, set ACCESS_DEV_EMAIL in apps/admin/.dev.vars.
        </p>
      </main>
    )
  }

  return (
    <SidebarProvider>
      <AppSidebar email={identity.data.email} />
      <SidebarInset>
        <Outlet />
      </SidebarInset>
    </SidebarProvider>
  )
}
