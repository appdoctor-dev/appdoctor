import { Link, useNavigate } from "@tanstack/react-router"
import { useEffect, useId, useMemo, useRef, useState } from "react"
import { DOC_GROUPS, type Heading } from "#/content/collections"
import { GUIDES } from "#/content/guides"
import { SITE } from "#/content/site"

// The docs, laid out like a developer docs site: a sidebar of groups, the
// page with its actions (copy as Markdown, open in an assistant, connect the
// MCP server), its contents on the right, and search on ⌘K.

export type DocLink = { slug: string; title: string; group: string }
export type SearchEntry = DocLink & { description: string; headings: Heading[] }

function Sidebar({ docs, current }: { docs: DocLink[]; current: string | null }) {
  const item = (active: boolean) =>
    `-ml-px block border-l py-1.5 pl-4 text-[14px] leading-snug ${active ? "border-accent font-medium text-accent" : "border-transparent text-ink/75 hover:border-ink/30 hover:text-ink"}`
  return (
    <nav aria-label="Docs" className="grid gap-7">
      <ul className="grid border-l border-line">
        <li>
          <Link to="/docs" className={item(current === null)} aria-current={current === null ? "page" : undefined}>
            Overview
          </Link>
        </li>
      </ul>
      {DOC_GROUPS.map((group) => {
        const pages = docs.filter((d) => d.group === group)
        if (!pages.length) return null
        return (
          <div key={group}>
            <p className="mb-2 text-[12px] font-semibold text-ink">{group}</p>
            <ul className="grid border-l border-line">
              {pages.map((doc) => (
                <li key={doc.slug}>
                  <Link to="/docs/$slug" params={{ slug: doc.slug }} className={item(doc.slug === current)} aria-current={doc.slug === current ? "page" : undefined}>
                    {doc.title}
                  </Link>
                </li>
              ))}
            </ul>
          </div>
        )
      })}
      <div>
        <p className="mb-2 text-[12px] font-semibold text-ink">Guides</p>
        <ul className="grid border-l border-line">
          {GUIDES.map((guide) => (
            <li key={guide.path}>
              <Link to={guide.path} className={item(false)}>
                {guide.h1}
              </Link>
            </li>
          ))}
        </ul>
      </div>
    </nav>
  )
}

/* ------------------------------------------------------------------------ */
/* Search                                                                    */
/* ------------------------------------------------------------------------ */

type Hit = { entry: SearchEntry; heading: Heading | null; score: number }

/** Pages ranked against the query: every word must appear; titles count most, then headings. */
export function searchDocs(entries: SearchEntry[], query: string): Hit[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean)
  if (!words.length) return []
  const hits: Hit[] = []
  for (const entry of entries) {
    let score = 0
    let heading: Heading | null = null
    let all = true
    for (const word of words) {
      const inHeading = entry.headings.find((h) => h.text.toLowerCase().includes(word))
      const points =
        (entry.title.toLowerCase().includes(word) ? 10 : 0) +
        (inHeading ? 4 : 0) +
        (entry.group.toLowerCase().includes(word) ? 2 : 0) +
        (entry.description.toLowerCase().includes(word) ? 1 : 0)
      if (!points) {
        all = false
        break
      }
      if (inHeading && !heading && !entry.title.toLowerCase().includes(word)) heading = inHeading
      score += points
    }
    if (all) hits.push({ entry, heading, score })
  }
  return hits.sort((a, b) => b.score - a.score).slice(0, 10)
}

function DocsSearch({ entries }: { entries: SearchEntry[] }) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState("")
  const [selected, setSelected] = useState(0)
  const dialog = useRef<HTMLDialogElement>(null)
  const navigate = useNavigate()
  const listId = useId()
  const hits = useMemo(() => searchDocs(entries, query), [entries, query])

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault()
        setOpen(true)
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [])
  useEffect(() => {
    if (open && !dialog.current?.open) dialog.current?.showModal()
    if (!open && dialog.current?.open) dialog.current.close()
  }, [open])
  useEffect(() => setSelected(0), [query])

  const go = (hit: Hit) => {
    setOpen(false)
    setQuery("")
    navigate({ to: "/docs/$slug", params: { slug: hit.entry.slug }, hash: hit.heading?.id })
  }

  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        className="flex h-9 w-full items-center gap-2 rounded-lg bg-surface px-3 text-left text-[13px] text-ink-soft hover:bg-[#ececf0]"
      >
        <svg viewBox="0 0 20 20" className="size-4" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8">
          <circle cx="8.5" cy="8.5" r="5.5" />
          <path d="m13 13 4 4" strokeLinecap="round" />
        </svg>
        Search docs
        <kbd className="ml-auto font-sans text-[12px]">⌘K</kbd>
      </button>
      <dialog
        ref={dialog}
        onClose={() => setOpen(false)}
        onClick={(e) => e.target === dialog.current && setOpen(false)}
        className="m-auto mt-[12vh] w-[min(600px,calc(100%-2rem))] rounded-2xl bg-white p-0 shadow-[0_30px_80px_-20px_rgb(0_0_0/0.4)] backdrop:bg-black/30 backdrop:backdrop-blur-sm"
        aria-label="Search docs"
      >
        <div className="flex items-center gap-3 border-b border-line px-4">
          <svg viewBox="0 0 20 20" className="size-5 shrink-0 text-ink-soft" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.8">
            <circle cx="8.5" cy="8.5" r="5.5" />
            <path d="m13 13 4 4" strokeLinecap="round" />
          </svg>
          <input
            autoFocus
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "ArrowDown") {
                e.preventDefault()
                setSelected((i) => Math.min(i + 1, hits.length - 1))
              } else if (e.key === "ArrowUp") {
                e.preventDefault()
                setSelected((i) => Math.max(i - 1, 0))
              } else if (e.key === "Enter" && hits[selected]) {
                e.preventDefault()
                go(hits[selected])
              }
            }}
            placeholder="Search the docs"
            aria-controls={listId}
            aria-activedescendant={hits[selected] ? `${listId}-${selected}` : undefined}
            className="h-14 w-full bg-transparent text-[17px] outline-none placeholder:text-ink-soft"
          />
        </div>
        <ul id={listId} role="listbox" aria-label="Results" className="max-h-[50vh] overflow-y-auto p-2">
          {query.trim() && hits.length === 0 && <li className="px-3 py-6 text-center text-ink-soft">Nothing matches “{query.trim()}”.</li>}
          {!query.trim() && <li className="px-3 py-6 text-center text-ink-soft">Search titles and sections, like “Apple Ads” or “video”.</li>}
          {hits.map((hit, i) => (
            <li
              key={`${hit.entry.slug}-${hit.heading?.id ?? ""}`}
              id={`${listId}-${i}`}
              role="option"
              aria-selected={i === selected}
              onMouseEnter={() => setSelected(i)}
              onClick={() => go(hit)}
              className={`cursor-pointer rounded-lg px-3 py-2.5 ${i === selected ? "bg-accent text-white" : ""}`}
            >
              <div className="text-[15px] font-medium">
                {hit.entry.title}
                {hit.heading && <span className={i === selected ? "text-white/80" : "text-ink-soft"}> › {hit.heading.text}</span>}
              </div>
              <div className={`truncate text-[13px] ${i === selected ? "text-white/80" : "text-ink-soft"}`}>{hit.entry.description}</div>
            </li>
          ))}
        </ul>
      </dialog>
    </>
  )
}

/* ------------------------------------------------------------------------ */
/* Page actions                                                              */
/* ------------------------------------------------------------------------ */

// AppDoctor's MCP server runs on the user's computer, so the install links
// carry the command, not a URL. This is where the Mac app installs.
const MCP_COMMAND = "/Applications/AppDoctor.app/Contents/MacOS/appdoctor"

function base64(text: string) {
  let binary = ""
  for (const byte of new TextEncoder().encode(text)) binary += String.fromCharCode(byte)
  return btoa(binary)
}

function actions(markdownUrl: string) {
  const prompt = encodeURIComponent(`Read ${markdownUrl} so I can ask questions about it.`)
  const server = { command: MCP_COMMAND, args: ["mcp"] }
  return [
    { id: "chatgpt", label: "Open in ChatGPT", detail: "Ask ChatGPT about this page", href: `https://chatgpt.com/?hints=search&q=${prompt}` },
    { id: "claude", label: "Open in Claude", detail: "Ask Claude about this page", href: `https://claude.ai/new?q=${prompt}` },
    { id: "perplexity", label: "Open in Perplexity", detail: "Ask Perplexity about this page", href: `https://www.perplexity.ai/search/new?q=${prompt}` },
    { id: "gemini", label: "Open in Gemini", detail: "Ask Gemini about this page", href: `https://gemini.google.com/app?q=${prompt}` },
    {
      id: "cursor",
      label: "Connect AppDoctor to Cursor",
      detail: "Add AppDoctor's MCP server (Mac)",
      href: `cursor://anysphere.cursor-deeplink/mcp/install?name=appdoctor&config=${encodeURIComponent(base64(JSON.stringify(server)))}`,
    },
    {
      id: "vscode",
      label: "Connect AppDoctor to VS Code",
      detail: "Add AppDoctor's MCP server (Mac)",
      href: `vscode:mcp/install?${encodeURIComponent(JSON.stringify({ name: "appdoctor", ...server }))}`,
    },
  ]
}

function PageActions({ markdown, markdownUrl }: { markdown: string; markdownUrl: string }) {
  const [open, setOpen] = useState(false)
  const [copied, setCopied] = useState(false)
  const root = useRef<HTMLDivElement>(null)
  const menuId = useId()

  useEffect(() => {
    if (!copied) return
    const t = setTimeout(() => setCopied(false), 1800)
    return () => clearTimeout(t)
  }, [copied])
  useEffect(() => {
    if (!open) return
    const onPointer = (e: PointerEvent) => !root.current?.contains(e.target as Node) && setOpen(false)
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false)
    document.addEventListener("pointerdown", onPointer)
    document.addEventListener("keydown", onKey)
    return () => {
      document.removeEventListener("pointerdown", onPointer)
      document.removeEventListener("keydown", onKey)
    }
  }, [open])

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(markdown)
      setCopied(true)
    } catch {
      window.open(markdownUrl, "_blank")
    }
  }

  return (
    <div ref={root} className="relative inline-flex rounded-lg border border-line text-[13px]">
      <button type="button" onClick={copy} className="flex h-8 items-center gap-1.5 rounded-l-lg px-3 hover:bg-surface" aria-live="polite">
        <svg viewBox="0 0 16 16" className="size-3.5" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.5">
          {copied ? <path d="m3.5 8.5 3 3 6-7" strokeLinecap="round" /> : <><rect x="5" y="5" width="8" height="9" rx="1.5" /><path d="M3 11V3.5A1.5 1.5 0 0 1 4.5 2H10" /></>}
        </svg>
        {copied ? "Copied" : "Copy page"}
      </button>
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={menuId}
        aria-label="More ways to use this page"
        onClick={() => setOpen((o) => !o)}
        className="flex h-8 items-center rounded-r-lg border-l border-line px-2 hover:bg-surface"
      >
        <svg viewBox="0 0 16 16" className="size-3.5" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.6">
          <path d="m4 6 4 4 4-4" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      {open && (
        <div id={menuId} role="menu" className="absolute top-10 right-0 z-30 w-72 rounded-xl border border-line bg-white p-1.5 shadow-[0_20px_50px_-12px_rgb(0_0_0/0.25)]">
          <a role="menuitem" href={markdownUrl} className="block rounded-lg px-3 py-2 hover:bg-surface">
            <span className="block font-medium">View as Markdown</span>
            <span className="block text-[12px] text-ink-soft">The page as plain text, for AI tools</span>
          </a>
          {actions(markdownUrl).map((action) => (
            <a key={action.id} role="menuitem" href={action.href} target={action.href.startsWith("http") ? "_blank" : undefined} rel="noreferrer" className="block rounded-lg px-3 py-2 hover:bg-surface">
              <span className="block font-medium">{action.label}</span>
              <span className="block text-[12px] text-ink-soft">{action.detail}</span>
            </a>
          ))}
        </div>
      )}
    </div>
  )
}

/** Copy buttons on code blocks. */
function useCopyButtons(ref: React.RefObject<HTMLElement | null>, key: string) {
  useEffect(() => {
    const root = ref.current
    if (!root) return
    for (const pre of root.querySelectorAll("pre")) {
      if (pre.querySelector(".copy-code")) continue
      const button = document.createElement("button")
      button.type = "button"
      button.className = "copy-code"
      button.textContent = "Copy"
      button.addEventListener("click", async () => {
        await navigator.clipboard.writeText(pre.querySelector("code")?.textContent ?? "")
        button.textContent = "Copied"
        setTimeout(() => (button.textContent = "Copy"), 1800)
      })
      pre.appendChild(button)
    }
  }, [ref, key])
}

const dated = (iso: string) => {
  const [y, m, d] = iso.split("-").map(Number)
  return new Date(Date.UTC(y!, (m ?? 1) - 1, d ?? 1)).toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" })
}

/* ------------------------------------------------------------------------ */
/* Layout                                                                    */
/* ------------------------------------------------------------------------ */

export type DocPageData = {
  slug: string
  title: string
  description: string
  group: string
  updated: string | null
  html: string
  markdown: string
  headings: Heading[]
  previous: { slug: string; title: string } | null
  next: { slug: string; title: string } | null
}

export function DocsLayout({ docs, search, current, children, contents }: { docs: DocLink[]; search: SearchEntry[]; current: string | null; children: React.ReactNode; contents?: Heading[] }) {
  return (
    <div className="mx-auto grid max-w-[1280px] gap-10 px-5 py-10 sm:py-14 lg:grid-cols-[15rem_minmax(0,1fr)] xl:grid-cols-[15rem_minmax(0,1fr)_13rem]">
      <aside className="lg:sticky lg:top-[84px] lg:max-h-[calc(100svh-100px)] lg:self-start lg:overflow-y-auto">
        <DocsSearch entries={search} />
        <div className="mt-7 hidden lg:block">
          <Sidebar docs={docs} current={current} />
        </div>
        <details className="group mt-3 rounded-lg border border-line lg:hidden">
          <summary className="flex cursor-pointer list-none items-center justify-between px-3 py-2 text-[14px] font-medium">
            Browse the docs
            <span className="text-ink-soft transition-transform group-open:rotate-180" aria-hidden="true">
              ⌄
            </span>
          </summary>
          <div className="border-t border-line p-3">
            <Sidebar docs={docs} current={current} />
          </div>
        </details>
      </aside>
      <div className="min-w-0">{children}</div>
      {contents && contents.filter((h) => h.depth === 2).length > 1 && (
        <nav aria-label="On this page" className="hidden xl:block">
          <div className="sticky top-[84px] text-[13px]">
            <p className="font-semibold">On this page</p>
            <ul className="mt-3 grid gap-2 border-l border-line">
              {contents
                .filter((h) => h.depth === 2)
                .map((h) => (
                  <li key={h.id}>
                    <a href={`#${h.id}`} className="-ml-px block border-l border-transparent pl-3 text-ink-soft hover:border-ink/30 hover:text-ink">
                      {h.text}
                    </a>
                  </li>
                ))}
            </ul>
          </div>
        </nav>
      )}
    </div>
  )
}

export function DocArticle({ page }: { page: DocPageData }) {
  const body = useRef<HTMLDivElement>(null)
  useCopyButtons(body, page.slug)
  const markdownUrl = `${SITE.url}/docs/${page.slug}.md`
  return (
    <article className="max-w-[720px]">
      <nav aria-label="Breadcrumb" className="text-[13px] text-ink-soft">
        <Link to="/docs" className="hover:text-ink">
          Docs
        </Link>
        {page.group && <span> / {page.group}</span>}
      </nav>
      <div className="mt-3 flex flex-wrap items-start justify-between gap-4">
        <h1 className="text-[2.4rem] leading-[1.1] font-semibold tracking-[-0.025em]">{page.title}</h1>
        <PageActions markdown={`# ${page.title}\n\n> ${page.description}\n\n${page.markdown}\n`} markdownUrl={markdownUrl} />
      </div>
      <p className="mt-3 text-[19px] leading-relaxed text-ink-soft">{page.description}</p>
      {page.updated && (
        <p className="mt-2 text-[13px] text-ink-soft">
          Updated <time dateTime={page.updated}>{dated(page.updated)}</time>
        </p>
      )}
      <div ref={body} className="prose-doc mt-8" dangerouslySetInnerHTML={{ __html: page.html }} />
      <nav aria-label="More docs" className="mt-16 grid gap-4 border-t border-line pt-8 sm:grid-cols-2">
        {page.previous ? (
          <Link to="/docs/$slug" params={{ slug: page.previous.slug }} className="rounded-xl border border-line p-4 hover:border-accent">
            <span className="block text-[12px] text-ink-soft">Previous</span>
            <span className="mt-1 block font-medium text-accent">{page.previous.title}</span>
          </Link>
        ) : (
          <span />
        )}
        {page.next && (
          <Link to="/docs/$slug" params={{ slug: page.next.slug }} className="rounded-xl border border-line p-4 text-right hover:border-accent">
            <span className="block text-[12px] text-ink-soft">Next</span>
            <span className="mt-1 block font-medium text-accent">{page.next.title}</span>
          </Link>
        )}
      </nav>
    </article>
  )
}
