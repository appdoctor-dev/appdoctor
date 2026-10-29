import { Link, createFileRoute, notFound } from "@tanstack/react-router"
import { SITE } from "#/content/site"
import { getDoc, getDocs } from "#/lib/data.functions"

export const Route = createFileRoute("/docs/$slug")({
  loader: async ({ params }) => {
    const [doc, docs] = await Promise.all([getDoc({ data: params.slug }), getDocs()])
    if (!doc) throw notFound()
    return { doc, docs }
  },
  head: ({ loaderData }) => ({
    meta: loaderData
      ? [{ title: `${loaderData.doc.title}: ${SITE.name} docs` }, { name: "description", content: loaderData.doc.description }]
      : [],
  }),
  component: DocPage,
})

function DocPage() {
  const { doc, docs } = Route.useLoaderData()
  return (
    <div className="mx-auto grid max-w-6xl gap-10 px-4 py-14 sm:px-6 lg:grid-cols-[220px_1fr]">
      <nav aria-label="Docs" className="text-sm lg:sticky lg:top-6 lg:self-start">
        <ul className="flex flex-col gap-1">
          {docs.map((d) => (
            <li key={d.slug}>
              <Link
                to="/docs/$slug"
                params={{ slug: d.slug }}
                className="block rounded-md px-2 py-1.5 text-ink-soft hover:text-ink"
                activeProps={{ className: "bg-surface font-medium text-ink" }}
              >
                {d.title}
              </Link>
            </li>
          ))}
        </ul>
      </nav>
      <article className="max-w-[68ch]">
        <h1 className="text-4xl font-bold tracking-tight">{doc.title}</h1>
        <p className="mt-3 text-lg text-ink-soft">{doc.description}</p>
        <div className="prose-doc mt-8" dangerouslySetInnerHTML={{ __html: doc.html }} />
      </article>
    </div>
  )
}
