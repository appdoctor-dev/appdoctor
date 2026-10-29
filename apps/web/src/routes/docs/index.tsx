import { Link, createFileRoute } from "@tanstack/react-router"
import { PageIntro } from "#/components/chrome"
import { SITE } from "#/content/site"
import { getDocs } from "#/lib/data.functions"

export const Route = createFileRoute("/docs/")({
  loader: () => getDocs(),
  head: () => ({
    meta: [
      { title: `Docs: ${SITE.name}` },
      { name: "description", content: `How to set up and use ${SITE.name}, the ASO tool for keyword research and rank tracking.` },
    ],
  }),
  component: DocsIndex,
})

function DocsIndex() {
  const docs = Route.useLoaderData()
  return (
    <>
      <PageIntro title="Docs">
        Setting up {SITE.name}, connecting Apple accounts, and what the numbers mean. Every page is also available as
        Markdown for AI agents: add <code>.md</code> to its address.
      </PageIntro>
      <ol className="mx-auto grid max-w-6xl gap-4 px-4 pb-20 sm:px-6 md:grid-cols-2">
        {docs.map((doc) => (
          <li key={doc.slug}>
            <Link to="/docs/$slug" params={{ slug: doc.slug }} className="block h-full rounded-2xl border border-line bg-surface p-6 hover:border-ink/30">
              <h2 className="text-lg font-semibold">{doc.title}</h2>
              <p className="mt-1 text-ink-soft">{doc.description}</p>
            </Link>
          </li>
        ))}
      </ol>
    </>
  )
}
