import { Link } from "@tanstack/react-router"
import { buttonClass } from "#/components/chrome"
import { GUIDES, type Block, type Guide } from "#/content/guides"
import { SITE } from "#/content/site"

// A long-form guide: breadcrumb, the answer up front, contents, sections,
// questions, then the other guide and a way to try AppDoctor.

export function guideHead(guide: Guide) {
  return [
    { title: guide.title },
    { name: "description", content: guide.description },
    { property: "og:title", content: guide.title },
    { property: "og:description", content: guide.description },
    { property: "og:type", content: "article" },
    { property: "article:modified_time", content: guide.updated },
  ]
}

const dated = (iso: string) =>
  new Date(`${iso}T12:00:00Z`).toLocaleDateString("en-GB", { day: "numeric", month: "long", year: "numeric", timeZone: "UTC" })

function BlockView({ block }: { block: Block }) {
  switch (block.kind) {
    case "p":
      return <p>{block.text}</p>
    case "list":
      return (
        <ul className="my-5 flex max-w-[68ch] flex-col gap-2.5">
          {block.items.map((item) => (
            <li key={item} className="flex gap-3">
              <span className="mt-[0.7em] size-1.5 shrink-0 rounded-full bg-ink-soft" aria-hidden="true" />
              {item}
            </li>
          ))}
        </ul>
      )
    case "steps":
      return (
        <ol className="my-6 flex max-w-[68ch] flex-col gap-7">
          {block.items.map((step, i) => (
            <li key={step.title} className="grid grid-cols-[2.25rem_1fr] gap-x-3">
              <span className="grid size-8 place-items-center rounded-full border border-ink text-[15px] font-semibold" aria-hidden="true">
                {i + 1}
              </span>
              <div>
                <h3 className="text-[1.1rem] leading-8 font-semibold">{step.title}</h3>
                <p className="!my-1 text-ink-soft">{step.text}</p>
              </div>
            </li>
          ))}
        </ol>
      )
    case "table":
      return (
        <div className="-mx-4 overflow-x-auto px-4">
          <table>
            <caption>{block.caption}</caption>
            <thead>
              <tr>
                {block.head.map((h, i) => (
                  <th key={i} scope="col">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {block.rows.map((row) => (
                <tr key={row[0]}>
                  {row.map((cell, i) =>
                    i === 0 ? (
                      <th key={i} scope="row">
                        {cell}
                      </th>
                    ) : (
                      <td key={i} className="text-ink-soft">
                        {cell}
                      </td>
                    ),
                  )}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )
  }
}

export function GuidePage({ guide }: { guide: Guide }) {
  const other = GUIDES.find((g) => g.path !== guide.path)!
  return (
    <article className="guide mx-auto max-w-6xl px-4 pt-10 pb-24 sm:px-6">
      <nav aria-label="Breadcrumb" className="text-sm text-ink-soft">
        <Link to="/" className="hover:text-ink">
          {SITE.name}
        </Link>
        <span aria-hidden="true"> / </span>
        <span aria-current="page">{guide.h1}</span>
      </nav>
      <div className="grid gap-x-16 lg:grid-cols-[minmax(0,1fr)_220px]">
        <div>
          <h1 className="mt-6 max-w-[18ch] text-[2.6rem] leading-[1.05] font-bold tracking-tight text-balance sm:text-[3.4rem]">{guide.h1}</h1>
          <p className="!mt-4 text-sm text-ink-soft">
            Updated <time dateTime={guide.updated}>{dated(guide.updated)}</time>
          </p>
          <div className="mt-6 text-[1.2rem] leading-relaxed">
            {guide.intro.map((p) => (
              <p key={p}>{p}</p>
            ))}
          </div>
          {guide.sections.map((section) => (
            <section key={section.id} aria-labelledby={section.id}>
              <h2 id={section.id}>{section.heading}</h2>
              {section.blocks.map((block, i) => (
                <BlockView key={i} block={block} />
              ))}
            </section>
          ))}
          <section aria-labelledby="questions">
            <h2 id="questions">Questions</h2>
            <div className="max-w-[68ch] divide-y divide-line border-y border-line">
              {guide.faq.map((f) => (
                <details key={f.q} className="group py-4">
                  <summary className="flex cursor-pointer list-none items-center justify-between gap-4 font-semibold">
                    {f.q}
                    <span className="text-ink-soft transition-transform group-open:rotate-45" aria-hidden="true">
                      +
                    </span>
                  </summary>
                  <p className="!mb-0 text-ink-soft">{f.a}</p>
                </details>
              ))}
            </div>
          </section>
          <aside className="mt-16 flex max-w-[68ch] flex-col gap-5 border-t-2 border-ink pt-8 sm:flex-row sm:items-end sm:justify-between">
            <div>
              <p className="!my-0 text-xl font-semibold">Try it on your own app</p>
              <p className="!my-1 text-ink-soft">AppDoctor runs this research on your Mac or PC, with Apple's numbers.</p>
            </div>
            <Link to="/download" className={buttonClass("primary")}>
              Download AppDoctor
            </Link>
          </aside>
          <p className="mt-10 text-ink-soft">
            Next:{" "}
            <Link to={other.path} className="font-semibold text-ink underline underline-offset-4">
              {other.h1}
            </Link>
          </p>
        </div>
        <nav aria-label="On this page" className="hidden lg:block">
          <div className="sticky top-24 mt-8 border-l border-line pl-4 text-sm">
            <p className="!my-0 mb-2 font-semibold">On this page</p>
            <ul className="flex flex-col gap-2 text-ink-soft">
              {guide.sections.map((s) => (
                <li key={s.id}>
                  <a href={`#${s.id}`} className="hover:text-ink">
                    {s.heading}
                  </a>
                </li>
              ))}
              <li>
                <a href="#questions" className="hover:text-ink">
                  Questions
                </a>
              </li>
            </ul>
          </div>
        </nav>
      </div>
    </article>
  )
}
