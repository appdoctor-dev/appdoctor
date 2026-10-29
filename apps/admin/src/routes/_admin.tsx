import { Navigate, Outlet, createFileRoute } from "@tanstack/react-router"
import { Loader2Icon } from "lucide-react"
import { AppSidebar } from "@/components/app-sidebar"
import { Button } from "@/components/ui/button"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { authClient } from "@/lib/api"

// The admin is a client-rendered app behind the API's session; nothing here is SSR'd.
export const Route = createFileRoute("/_admin")({ ssr: false, component: AdminLayout })

function AdminLayout() {
  const { data: session, isPending } = authClient.useSession()

  if (isPending) {
    return (
      <div className="flex min-h-svh items-center justify-center">
        <Loader2Icon className="size-5 animate-spin text-muted-foreground" />
      </div>
    )
  }
  if (!session) return <Navigate to="/login" />
  if (session.user.role !== "admin") {
    return (
      <main className="flex min-h-svh flex-col items-center justify-center gap-3 p-4 text-center">
        <p className="font-medium">{session.user.email} is not an admin.</p>
        <p className="max-w-sm text-sm text-muted-foreground">
          Add the address to ADMIN_EMAILS before it first signs in, or give it the admin role.
        </p>
        <Button variant="outline" onClick={() => authClient.signOut()}>
          Sign out
        </Button>
      </main>
    )
  }

  return (
    <SidebarProvider>
      <AppSidebar email={session.user.email} />
      <SidebarInset>
        <Outlet />
      </SidebarInset>
    </SidebarProvider>
  )
}
