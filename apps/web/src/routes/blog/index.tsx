import { Link, createFileRoute } from "@tanstack/react-router"
import { PageIntro } from "#/components/chrome"
import { SITE } from "#/content/site"
import { getPosts } from "#/lib/data.functions"
import { date } from "#/lib/format"

export const Route = createFileRoute("/blog/")({
  loader: () => getPosts(),
  head: () => ({
    meta: [
      { title: `Blog: ${SITE.name}` },
      { name: "description", content: "App Store Optimization, keyword research and news about ASO tools." },
    ],
  }),
  component: BlogIndex,
})

function BlogIndex() {
  const posts = Route.useLoaderData()
  return (
    <>
      <PageIntro title="Blog">App Store Optimization, keyword research, and what's new in {SITE.name}.</PageIntro>
      <ul className="mx-auto flex max-w-3xl flex-col divide-y divide-line px-4 pb-20 sm:px-6">
        {posts.map((post) => (
          <li key={post.slug} className="py-6">
            <time dateTime={post.date ?? undefined} className="text-sm text-ink-soft">
              {date(post.date)}
            </time>
            <h2 className="mt-1 text-xl font-semibold">
              <Link to="/blog/$slug" params={{ slug: post.slug }} className="hover:underline">
                {post.title}
              </Link>
            </h2>
            <p className="mt-2 text-ink-soft">{post.description}</p>
          </li>
        ))}
      </ul>
    </>
  )
}
