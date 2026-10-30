import { Marked, type Tokens } from "marked"

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
  /** The docs sidebar group it's listed under. */
  group: string
  markdown: string
}

export type Heading = { depth: number; text: string; id: string }

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
    group: meta.group ?? "",
    markdown: raw.slice(match?.[0].length ?? 0).trim(),
  }
}

export const POSTS = Object.entries(blogFiles)
  .map(([path, raw]) => parse(path, raw as string))
  .sort((a, b) => (b.date ?? "").localeCompare(a.date ?? ""))

export const DOCS = Object.entries(docFiles)
  .map(([path, raw]) => parse(path, raw as string))
  .sort((a, b) => a.order - b.order)

/** The docs sidebar, in order: each group with its pages. */
export const DOC_GROUPS = ["Start", "Connect your accounts", "Keywords and ranks", "Create", "AI"]

/** A heading's anchor: "Connect an agent" → "connect-an-agent". */
export function slugify(text: string) {
  return text
    .toLowerCase()
    .replace(/<[^>]+>|[`*_]/g, "")
    .replace(/[^\p{L}\p{N}]+/gu, "-")
    .replace(/^-+|-+$/g, "")
}

// Headings get ids, so the "On this page" list and shared links can point at them.
const markdown = new Marked({ gfm: true }).use({
  renderer: {
    heading({ tokens, depth, text }: Tokens.Heading) {
      return `<h${depth} id="${slugify(text)}">${this.parser.parseInline(tokens)}</h${depth}>\n`
    },
  },
})

export function renderMarkdown(source: string) {
  return markdown.parse(source, { async: false })
}

/** The page's second- and third-level headings, for its contents list. */
export function headingsOf(source: string): Heading[] {
  return markdown
    .lexer(source)
    .filter((t): t is Tokens.Heading => t.type === "heading" && (t.depth === 2 || t.depth === 3))
    .map((t) => ({ depth: t.depth, text: t.text.replace(/[`*_]/g, ""), id: slugify(t.text) }))
}
