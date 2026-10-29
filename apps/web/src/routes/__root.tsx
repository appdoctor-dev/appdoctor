import { HeadContent, Scripts, createRootRoute } from "@tanstack/react-router"
import { SiteFooter, SiteHeader } from "#/components/chrome"
import { SITE } from "#/content/site"
import appCss from "../styles.css?url"

const organization = {
  "@context": "https://schema.org",
  "@type": "SoftwareApplication",
  name: SITE.name,
  description: SITE.description,
  applicationCategory: "BusinessApplication",
  operatingSystem: "macOS, Windows",
  offers: { "@type": "Offer", category: "subscription" },
  isAccessibleForFree: false,
  codeRepository: SITE.sourceUrl,
}

export const Route = createRootRoute({
  head: () => ({
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      { title: SITE.title },
      { name: "description", content: SITE.description },
      { property: "og:site_name", content: SITE.name },
      { property: "og:type", content: "website" },
      { name: "theme-color", content: "#0f1b2d" },
    ],
    links: [
      { rel: "stylesheet", href: appCss },
      { rel: "manifest", href: "/manifest.webmanifest" },
      { rel: "icon", type: "image/svg+xml", href: "/favicon.svg" },
      { rel: "alternate", type: "application/rss+xml", title: `${SITE.name} blog`, href: "/blog/rss.xml" },
      { rel: "help", type: "text/plain", href: "/llms.txt" },
    ],
    scripts: [{ type: "application/ld+json", children: JSON.stringify(organization) }],
  }),
  shellComponent: RootDocument,
})

function RootDocument({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <head>
        <HeadContent />
      </head>
      <body className="flex min-h-svh flex-col">
        <SiteHeader />
        <main className="flex-1">{children}</main>
        <SiteFooter />
        <Scripts />
      </body>
    </html>
  )
}
