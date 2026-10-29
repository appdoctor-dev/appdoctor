import { Link } from "@tanstack/react-router"
import { NAV, SITE } from "#/content/site"
import { authClient } from "#/lib/auth"

export function SiteHeader() {
  const { data: session } = authClient.useSession()

  return (
    <header className="border-b border-line bg-paper/90 backdrop-blur supports-[backdrop-filter]:bg-paper/70">
      <div className="mx-auto flex h-16 max-w-6xl items-center gap-6 px-4 sm:px-6">
        <Link to="/" className="flex items-center gap-2 text-[17px] font-bold tracking-tight">
          <Mark />
          {SITE.name}
        </Link>
        <nav className="hidden items-center gap-5 text-sm text-ink-soft md:flex">
          {NAV.map((item) => (
            <Link key={item.to} to={item.to} className="hover:text-ink" activeProps={{ className: "text-ink" }}>
              {item.label}
            </Link>
          ))}
        </nav>
        <div className="ml-auto flex items-center gap-3 text-sm">
          {session ? (
            <Link to="/account" className="font-medium hover:underline">
              Account
            </Link>
          ) : (
            <Link to="/login" className="text-ink-soft hover:text-ink">
              Sign in
            </Link>
          )}
          <Link to="/download" className={buttonClass("primary", "sm")}>
            Download
          </Link>
        </div>
      </div>
    </header>
  )
}

export function SiteFooter() {
  return (
    <footer className="border-t border-line">
      <div className="mx-auto grid max-w-6xl gap-8 px-4 py-10 text-sm text-ink-soft sm:grid-cols-[2fr_1fr_1fr] sm:px-6">
        <p className="max-w-sm">
          {SITE.name} is an open-source ASO tool for Mac and Windows. Your keywords and rankings stay on your computer.
        </p>
        <ul className="flex flex-col gap-2">
          <li>
            <Link to="/pricing" className="hover:text-ink">
              Pricing
            </Link>
          </li>
          <li>
            <Link to="/docs" className="hover:text-ink">
              Docs
            </Link>
          </li>
          <li>
            <Link to="/blog" className="hover:text-ink">
              Blog
            </Link>
          </li>
        </ul>
        <ul className="flex flex-col gap-2">
          <li>
            <Link to="/privacy" className="hover:text-ink">
              Privacy
            </Link>
          </li>
          <li>
            <a href={SITE.sourceUrl} className="hover:text-ink">
              Source code
            </a>
          </li>
          <li>
            <a href="/llms.txt" className="hover:text-ink">
              For AI agents
            </a>
          </li>
        </ul>
      </div>
    </footer>
  )
}

/** A search field's magnifier with a pulse line through it: searching, checked. */
export function Mark({ className = "size-6" }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} aria-hidden="true">
      <circle cx="10" cy="10" r="7" fill="none" stroke="currentColor" strokeWidth="2" />
      <path d="M15.2 15.2 21 21" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" />
      <path
        d="M5 10.5h2.2l1.4-3 2 6 1.4-3H15"
        fill="none"
        stroke="var(--color-up)"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

export function buttonClass(variant: "primary" | "secondary" = "primary", size: "sm" | "md" = "md") {
  const base =
    "inline-flex items-center justify-center gap-2 rounded-lg font-semibold transition-colors disabled:opacity-50"
  const sizes = { sm: "h-9 px-3.5 text-sm", md: "h-11 px-5 text-[15px]" }
  const variants = {
    primary: "bg-ink text-paper hover:bg-ink/85",
    secondary: "border border-line bg-surface text-ink hover:border-ink/30",
  }
  return `${base} ${sizes[size]} ${variants[variant]}`
}

export function PageIntro({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="mx-auto max-w-6xl px-4 pt-14 pb-8 sm:px-6">
      <h1 className="max-w-3xl text-4xl font-bold tracking-tight text-balance sm:text-5xl">{title}</h1>
      {children && <div className="mt-4 max-w-2xl text-lg text-ink-soft">{children}</div>}
    </div>
  )
}
