import { Link, createFileRoute } from "@tanstack/react-router"
import { DocsLayout } from "#/components/docs"
import { DOC_GROUPS } from "#/content/collections"
import { SITE } from "#/content/site"
import { getDocs, getDocsSearch } from "#/lib/data.functions"

export const Route = createFileRoute("/docs/")({
  loader: async () => {
    const [docs, search] = await Promise.all([getDocs(), getDocsSearch()])
    return { docs, search }
  },
  head: () => ({
    meta: [
      { title: `Docs: ${SITE.name}, the ASO keyword research tool` },
      { name: "description", content: `How to set up and use ${SITE.name}: connect App Store Connect and Apple Ads, research keywords, track ranks, audit your listing, design screenshots and work with your own AI.` },
    ],
  }),
  component: DocsIndex,
})

function DocsIndex() {
  const { docs, search } = Route.useLoaderData()
  return (
    <DocsLayout docs={docs} search={search} current={null}>
      <div className="max-w-[820px]">
        <h1 className="text-[2.6rem] leading-[1.08] font-semibold tracking-[-0.03em]">{SITE.name} docs</h1>
        <p className="mt-4 max-w-[56ch] text-[19px] leading-relaxed text-ink-soft">
          Setting up {SITE.name}, connecting your Apple accounts, what the numbers mean, and working with your own AI. Every
          page is also plain Markdown for AI tools: add <code className="rounded bg-surface px-1">.md</code> to its address,
          or use Copy page.
        </p>
        <div className="mt-12 grid gap-10">
          {DOC_GROUPS.map((group) => {
            const pages = docs.filter((d) => d.group === group)
            if (!pages.length) return null
            return (
              <section key={group} aria-labelledby={`group-${group}`}>
                <h2 id={`group-${group}`} className="text-[13px] font-semibold text-ink-soft">
                  {group}
                </h2>
                <ul className="mt-3 grid gap-3 sm:grid-cols-2">
                  {pages.map((doc) => (
                    <li key={doc.slug}>
                      <Link to="/docs/$slug" params={{ slug: doc.slug }} className="block h-full rounded-2xl bg-surface p-5 transition-colors hover:bg-[#ececf0]">
                        <span className="block text-[17px] font-semibold">{doc.title}</span>
                        <span className="mt-1 block text-[14px] leading-relaxed text-ink-soft">{doc.description}</span>
                      </Link>
                    </li>
                  ))}
                </ul>
              </section>
            )
          })}
        </div>
      </div>
    </DocsLayout>
  )
}
