import { createFileRoute, notFound } from "@tanstack/react-router"
import { DocArticle, DocsLayout } from "#/components/docs"
import { SITE } from "#/content/site"
import { getDoc, getDocs, getDocsSearch } from "#/lib/data.functions"
import { breadcrumbs } from "#/lib/jsonld"

export const Route = createFileRoute("/docs/$slug")({
  loader: async ({ params }) => {
    const [doc, docs, search] = await Promise.all([getDoc({ data: params.slug }), getDocs(), getDocsSearch()])
    if (!doc) throw notFound()
    return { doc, docs, search }
  },
  head: ({ loaderData }) =>
    loaderData
      ? {
          meta: [
            { title: `${loaderData.doc.title}: ${SITE.name} docs` },
            { name: "description", content: loaderData.doc.description },
            { property: "og:title", content: loaderData.doc.title },
            { property: "og:description", content: loaderData.doc.description },
          ],
          scripts: [
            {
              type: "application/ld+json",
              children: JSON.stringify({
                "@context": "https://schema.org",
                "@graph": [
                  {
                    "@type": "TechArticle",
                    headline: loaderData.doc.title,
                    description: loaderData.doc.description,
                    ...(loaderData.doc.updated ? { dateModified: loaderData.doc.updated } : {}),
                    about: { "@id": `${SITE.url}/#software` },
                    publisher: { "@id": `${SITE.url}/#organization` },
                  },
                  breadcrumbs([
                    { name: SITE.name, path: "/" },
                    { name: "Docs", path: "/docs" },
                    { name: loaderData.doc.title, path: `/docs/${loaderData.doc.slug}` },
                  ]),
                ],
              }),
            },
          ],
        }
      : {},
  component: DocPage,
})

function DocPage() {
  const { doc, docs, search } = Route.useLoaderData()
  return (
    <DocsLayout docs={docs} search={search} current={doc.slug} contents={doc.headings}>
      <DocArticle page={doc} />
    </DocsLayout>
  )
}
