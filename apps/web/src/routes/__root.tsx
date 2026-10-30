import { HeadContent, Scripts, createRootRoute } from "@tanstack/react-router"
import { SiteFooter, SiteHeader } from "#/components/chrome"
import { SITE } from "#/content/site"
import { siteGraph } from "#/lib/jsonld"
import appCss from "../styles.css?url"

// Account pages: reachable by link, never indexed (the server also sends
// X-Robots-Tag), and with no Markdown version.
const PRIVATE = ["/account", "/device", "/login"]

export const Route = createRootRoute({
  head: ({ matches }) => {
    const path = (matches.at(-1)?.pathname ?? "/").replace(/\/+$/, "") || "/"
    const isPrivate = PRIVATE.some((p) => path.startsWith(p))
    const canonical = `${SITE.url}${path === "/" ? "/" : path}`
    return {
    meta: [
      { charSet: "utf-8" },
      { name: "viewport", content: "width=device-width, initial-scale=1" },
      { title: SITE.title },
      { name: "description", content: SITE.description },
      { property: "og:site_name", content: SITE.name },
      { property: "og:type", content: "website" },
      { property: "og:url", content: canonical },
      { name: "twitter:card", content: "summary" },
      { name: "theme-color", content: "#0f1b2d" },
      ...(isPrivate ? [{ name: "robots", content: "noindex" }] : []),
    ],
    links: [
      { rel: "stylesheet", href: appCss },
      { rel: "manifest", href: "/manifest.webmanifest" },
      { rel: "icon", type: "image/svg+xml", href: "/favicon.svg" },
      { rel: "alternate", type: "application/rss+xml", title: `${SITE.name} blog`, href: "/blog/rss.xml" },
      { rel: "help", type: "text/plain", href: "/llms.txt" },
      // The same page as Markdown, for agents (also in the Link header).
      ...(isPrivate
        ? []
        : [
            { rel: "canonical", href: canonical },
            { rel: "alternate", type: "text/markdown", href: path === "/" ? "/index.md" : `${path}.md` },
          ]),
    ],
    scripts: [{ type: "application/ld+json", children: JSON.stringify(siteGraph()) }],
    }
  },
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
