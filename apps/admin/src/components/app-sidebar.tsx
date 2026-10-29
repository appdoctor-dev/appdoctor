import { Link } from "@tanstack/react-router"
import {
  BellIcon,
  CreditCardIcon,
  LayoutDashboardIcon,
  LogOutIcon,
  PackageIcon,
  TagIcon,
  UsersIcon,
} from "lucide-react"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import { authClient } from "@/lib/api"

const nav = [
  { to: "/", label: "Dashboard", icon: LayoutDashboardIcon },
  { to: "/subscribers", label: "Subscribers", icon: CreditCardIcon },
  { to: "/users", label: "Users", icon: UsersIcon },
  { to: "/pricing", label: "Pricing", icon: TagIcon },
  { to: "/releases", label: "Releases", icon: PackageIcon },
  { to: "/alerts", label: "Alerts", icon: BellIcon },
] as const

export function AppSidebar({ email }: { email: string }) {
  return (
    <Sidebar>
      <SidebarHeader>
        <div className="px-2 py-1.5 text-sm font-semibold">AppDoctor Admin</div>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupContent>
            <SidebarMenu>
              {nav.map((item) => (
                <SidebarMenuItem key={item.to}>
                  <Link to={item.to} activeOptions={{ exact: item.to === "/" }}>
                    {({ isActive }) => (
                      <SidebarMenuButton isActive={isActive}>
                        <item.icon />
                        <span>{item.label}</span>
                      </SidebarMenuButton>
                    )}
                  </Link>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton onClick={() => authClient.signOut()} tooltip="Sign out">
              <LogOutIcon />
              <span className="truncate">{email}</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarFooter>
    </Sidebar>
  )
}
