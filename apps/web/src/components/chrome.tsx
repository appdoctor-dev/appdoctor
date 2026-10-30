import { Link } from "@tanstack/react-router"
import { NAV, SITE } from "#/content/site"
import { authClient } from "#/lib/auth"

/** Where sign-in and the account live (app.appdoctor.so); this site when unset, as locally. */
export const APP_URL = import.meta.env.VITE_APP_URL ?? ""

export function SiteHeader() {
  const { data: session } = authClient.useSession()

  return (
    <header className="sticky top-0 z-40 border-b border-black/[0.08] bg-white/80 backdrop-blur-xl supports-[backdrop-filter]:bg-white/70">
      <div className="mx-auto flex h-[52px] max-w-[1080px] items-center gap-7 px-5">
        <Link to="/" className="flex items-center gap-2 text-[15px] font-semibold tracking-[-0.01em]">
          <Mark className="size-[26px]" />
          {SITE.name}
        </Link>
        <nav aria-label="Site" className="hidden items-center gap-6 text-[13px] text-ink/80 md:flex">
          {NAV.map((item) => (
            <Link key={item.to} to={item.to} className="hover:text-ink" activeProps={{ className: "text-ink" }}>
              {item.label}
            </Link>
          ))}
        </nav>
        <div className="ml-auto flex items-center gap-4 text-[13px]">
          {session ? (
            <a href={`${APP_URL}/account`} className="text-ink/80 hover:text-ink">
              Account
            </a>
          ) : (
            <a href={`${APP_URL}/login`} className="text-ink/80 hover:text-ink">
              Sign in
            </a>
          )}
          <Link to="/download" className={buttonClass("primary", "sm")}>
            Download
          </Link>
          <details className="group relative md:hidden">
            <summary className="grid size-8 cursor-pointer list-none place-items-center rounded-full hover:bg-surface" aria-label="Menu">
              <svg viewBox="0 0 16 16" className="size-4" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
                <path d="M2.5 5h11M2.5 11h11" className="group-open:hidden" />
                <path d="m4 4 8 8M12 4l-8 8" className="hidden group-open:block" />
              </svg>
            </summary>
            <nav aria-label="Site" className="absolute top-10 right-0 w-56 rounded-2xl border border-line bg-white p-2 shadow-[0_20px_50px_-12px_rgb(0_0_0/0.25)]">
              {NAV.map((item) => (
                <Link key={item.to} to={item.to} className="block rounded-lg px-3 py-2.5 text-[15px] hover:bg-surface">
                  {item.label}
                </Link>
              ))}
            </nav>
          </details>
        </div>
      </div>
    </header>
  )
}

const FOOTER = [
  {
    title: "AppDoctor",
    links: [
      { to: "/download", label: "Download" },
      { to: "/pricing", label: "Pricing" },
      { to: "/blog", label: "Blog" },
    ],
  },
  {
    title: "Learn",
    links: [
      { to: "/aso-keyword-research", label: "ASO keyword research" },
      { to: "/aso-keyword-research-tools", label: "ASO keyword research tools" },
      { to: "/docs", label: "Docs" },
    ],
  },
  {
    title: "About",
    links: [
      { to: "/privacy", label: "Privacy" },
      { href: SITE.sourceUrl, label: "Source code" },
      { href: "/llms.txt", label: "For AI agents" },
    ],
  },
] as const

export function SiteFooter() {
  return (
    <footer className="bg-surface text-[12px] text-ink-soft">
      <div className="mx-auto max-w-[1080px] px-5 pt-8 pb-10">
        <p className="max-w-xl border-b border-line pb-4">
          {SITE.name} is an open-source ASO keyword research tool for Mac and Windows. Your keywords, rankings and keys
          stay on your computer.
        </p>
        <div className="grid grid-cols-2 gap-8 py-6 sm:grid-cols-3">
          {FOOTER.map((column) => (
            <div key={column.title}>
              <h2 className="mb-2.5 font-semibold text-ink">{column.title}</h2>
              <ul className="flex flex-col gap-2">
                {column.links.map((link) => (
                  <li key={link.label}>
                    {"to" in link ? (
                      <Link to={link.to} className="hover:text-ink hover:underline">
                        {link.label}
                      </Link>
                    ) : (
                      <a href={link.href} className="hover:text-ink hover:underline">
                        {link.label}
                      </a>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
        <p className="border-t border-line pt-4">Not affiliated with Apple. App Store is a trademark of Apple Inc.</p>
      </div>
    </footer>
  )
}

/** The app's icon: a stethoscope checking an app's rising chart. */
export function Mark({ className = "size-7" }: { className?: string }) {
  return <img src="/favicon.svg" alt="" aria-hidden="true" className={`shrink-0 ${className}`} />
}

export function buttonClass(variant: "primary" | "secondary" | "plain" = "primary", size: "sm" | "md" | "lg" = "md") {
  const base = "inline-flex items-center justify-center gap-2 rounded-full font-medium transition-colors disabled:opacity-50"
  const sizes = { sm: "h-7 px-3 text-[12px]", md: "h-10 px-5 text-[15px]", lg: "h-12 px-7 text-[17px]" }
  const variants = {
    primary: "bg-accent text-white hover:bg-accent-hover",
    secondary: "border border-accent text-accent hover:bg-accent hover:text-white",
    plain: "text-accent hover:underline",
  }
  return `${base} ${sizes[size]} ${variants[variant]}`
}

export function PageIntro({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="mx-auto max-w-[1080px] px-5 pt-16 pb-10 text-center sm:pt-20">
      <h1 className="mx-auto max-w-3xl text-[2.5rem] leading-[1.08] font-semibold tracking-[-0.025em] text-balance sm:text-[3.5rem]">{title}</h1>
      {children && <div className="mx-auto mt-4 max-w-2xl text-[19px] leading-relaxed text-ink-soft">{children}</div>}
    </div>
  )
}
