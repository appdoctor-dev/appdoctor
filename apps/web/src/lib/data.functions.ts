import { createServerFn } from "@tanstack/react-start"
import { DOCS, POSTS, renderMarkdown } from "#/content/collections"
import { fetchLatestBuilds, fetchPlans } from "#/server/api"

export const getPlans = createServerFn({ method: "GET" }).handler(() => fetchPlans())
export const getLatestBuilds = createServerFn({ method: "GET" }).handler(() => fetchLatestBuilds())

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
    const doc = DOCS.find((d) => d.slug === slug)
    return doc ? { ...meta(doc), html: renderMarkdown(doc.markdown) } : null
  })
