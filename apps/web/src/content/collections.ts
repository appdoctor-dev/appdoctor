import { marked } from "marked"

// Blog posts and docs are Markdown files in apps/web/content, bundled at build
// time. The raw Markdown is what agents get from the .md URLs; people get the HTML.

export type Entry = {
  slug: string
  title: string
  description: string
  date: string | null
  updated: string | null
  author: string | null
  order: number
  markdown: string
}

const blogFiles = import.meta.glob("/content/blog/*.md", { query: "?raw", import: "default", eager: true })
const docFiles = import.meta.glob("/content/docs/*.md", { query: "?raw", import: "default", eager: true })

function parse(path: string, raw: string): Entry {
  const slug = path.split("/").pop()!.replace(/\.md$/, "")
  const match = /^---\n([\s\S]*?)\n---\n?/.exec(raw)
  const meta: Record<string, string> = {}
  for (const line of (match?.[1] ?? "").split("\n")) {
    const i = line.indexOf(":")
    if (i > 0) meta[line.slice(0, i).trim()] = line.slice(i + 1).trim().replace(/^["']|["']$/g, "")
  }
  return {
    slug,
    title: meta.title ?? slug,
    description: meta.description ?? "",
    date: meta.date ?? null,
    updated: meta.updated ?? null,
    author: meta.author ?? null,
    order: Number(meta.order ?? 0),
    markdown: raw.slice(match?.[0].length ?? 0).trim(),
  }
}

export const POSTS = Object.entries(blogFiles)
  .map(([path, raw]) => parse(path, raw as string))
  .sort((a, b) => (b.date ?? "").localeCompare(a.date ?? ""))

export const DOCS = Object.entries(docFiles)
  .map(([path, raw]) => parse(path, raw as string))
  .sort((a, b) => a.order - b.order)

export function renderMarkdown(markdown: string) {
  return marked.parse(markdown, { async: false, gfm: true })
}
