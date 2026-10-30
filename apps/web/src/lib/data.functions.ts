import { createServerFn } from "@tanstack/react-start"
import { DOCS, POSTS, headingsOf, renderMarkdown } from "#/content/collections"
import { fetchLatestRelease, fetchPlans } from "#/server/api"

export const getPlans = createServerFn({ method: "GET" }).handler(() => fetchPlans())
export const getLatestRelease = createServerFn({ method: "GET" }).handler(() => fetchLatestRelease())

const meta = ({ markdown: _markdown, ...rest }: (typeof POSTS)[number]) => rest

export const getPosts = createServerFn({ method: "GET" }).handler(() => POSTS.map(meta))
export const getDocs = createServerFn({ method: "GET" }).handler(() => DOCS.map(meta))

export const getPost = createServerFn({ method: "GET" })
  .validator((slug: string) => slug)
  .handler(({ data: slug }) => {
    const post = POSTS.find((p) => p.slug === slug)
    return post ? { ...meta(post), html: renderMarkdown(post.markdown) } : null
  })

export const getDoc = createServerFn({ method: "GET" })
  .validator((slug: string) => slug)
  .handler(({ data: slug }) => {
    const index = DOCS.findIndex((d) => d.slug === slug)
    const doc = DOCS[index]
    if (!doc) return null
    const link = (d: (typeof DOCS)[number] | undefined) => (d ? { slug: d.slug, title: d.title } : null)
    return {
      ...meta(doc),
      html: renderMarkdown(doc.markdown),
      markdown: doc.markdown,
      headings: headingsOf(doc.markdown),
      previous: link(DOCS[index - 1]),
      next: link(DOCS[index + 1]),
    }
  })

/** Everything docs search looks through: titles, descriptions and headings. */
export const getDocsSearch = createServerFn({ method: "GET" }).handler(() =>
  DOCS.map((d) => ({ slug: d.slug, title: d.title, description: d.description, group: d.group, headings: headingsOf(d.markdown) })),
)
