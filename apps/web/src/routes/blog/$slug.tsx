import { createFileRoute, notFound } from "@tanstack/react-router"
import { SITE } from "#/content/site"
import { getPost } from "#/lib/data.functions"
import { date } from "#/lib/format"

export const Route = createFileRoute("/blog/$slug")({
  loader: async ({ params }) => {
    const post = await getPost({ data: params.slug })
    if (!post) throw notFound()
    return post
  },
  head: ({ loaderData: post }) =>
    post
      ? {
          meta: [
            { title: `${post.title}: ${SITE.name}` },
            { name: "description", content: post.description },
            { property: "og:type", content: "article" },
            { property: "og:title", content: post.title },
            { property: "og:description", content: post.description },
          ],
          scripts: [
            {
              type: "application/ld+json",
              children: JSON.stringify({
                "@context": "https://schema.org",
                "@type": "BlogPosting",
                headline: post.title,
                description: post.description,
                datePublished: post.date,
                dateModified: post.updated ?? post.date,
                author: { "@type": "Organization", name: post.author ?? SITE.name },
              }),
            },
          ],
        }
      : {},
  component: Post,
})

function Post() {
  const post = Route.useLoaderData()
  return (
    <article className="mx-auto max-w-[68ch] px-4 py-16">
      <time dateTime={post.date ?? undefined} className="text-sm text-ink-soft">
        {date(post.date)}
      </time>
      <h1 className="mt-2 text-4xl font-bold tracking-tight text-balance">{post.title}</h1>
      <p className="mt-4 text-lg text-ink-soft">{post.description}</p>
      <div className="prose-doc mt-10" dangerouslySetInnerHTML={{ __html: post.html }} />
    </article>
  )
}
