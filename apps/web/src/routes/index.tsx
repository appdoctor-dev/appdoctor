import { Link, createFileRoute } from "@tanstack/react-router"
import { useState } from "react"
import { AppWindow, type View } from "#/components/app-window"
import { buttonClass } from "#/components/chrome"
import { GUIDES } from "#/content/guides"
import { FAQ, FEATURES, PRIVACY_POINTS, SITE } from "#/content/site"
import { faqPage } from "#/lib/jsonld"

export const Route = createFileRoute("/")({
  head: () => ({
    meta: [
      { property: "og:title", content: SITE.title },
      { property: "og:description", content: SITE.description },
    ],
    scripts: [{ type: "application/ld+json", children: JSON.stringify({ "@context": "https://schema.org", ...faqPage(FAQ) }) }],
  }),
  component: Home,
})

/** The research loop, each step shown in the app window above. */
const STEPS: { title: string; body: string; view: View }[] = [
  {
    title: "Find the searches",
    body: "Add the words your app is for and AppDoctor checks each one: Apple's popularity score from your Apple Ads account, and how strong the apps in the top 10 are.",
    view: "keywords",
  },
  {
    title: "Pick the ones you can win",
    body: "Sort by popularity and difficulty, and compare countries. A keyword you can be third for brings more installs than one you'll never reach.",
    view: "keywords",
  },
  {
    title: "Put them in your listing",
    body: "Edit the name, subtitle and keyword field with live counts. AppDoctor shows which tracked keywords your listing can't match and which words you've wasted, then saves to App Store Connect.",
    view: "localization",
  },
  {
    title: "See what changed",
    body: "Ranks are checked in every country you track and kept as history, so a release that moved you from 22nd to 8th shows as a line, not a hunch.",
    view: "rankings",
  },
]

function Home() {
  const [view, setView] = useState<View>("keywords")
  const show = (next: View) => {
    setView(next)
    document.getElementById("app")?.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "center" })
  }
  const rest = FEATURES.filter((f) => !["keywords", "ranks", "listing"].includes(f.id))

  return (
    <>
      <section className="overflow-hidden">
        <div className="mx-auto max-w-6xl px-4 pt-16 sm:px-6 lg:pt-24">
          <h1 className="max-w-[15ch] text-[2.7rem] leading-[1.02] font-bold tracking-[-0.025em] text-balance sm:text-[4.4rem]">
            ASO keyword research with Apple's own numbers
          </h1>
          <div className="mt-7 grid gap-8 lg:grid-cols-[minmax(0,34rem)_1fr] lg:items-end">
            <p className="text-lg leading-relaxed text-ink-soft">
              {SITE.name} finds the App Store searches worth winning, tracks your rank in every country, audits your listing
              and lets your own AI fix it. It's an app for Mac and Windows, and your keywords stay on your computer.
            </p>
            <div className="flex flex-wrap gap-3 lg:justify-end">
              <Link to="/download" className={buttonClass("primary")}>
                Download for Mac or Windows
              </Link>
              <Link to="/aso-keyword-research" className={buttonClass("secondary")}>
                Read the keyword research guide
              </Link>
            </div>
          </div>
        </div>
        <div id="app" className="mx-auto mt-14 max-w-[1180px] px-3 sm:px-6">
          <AppWindow view={view} onView={setView} />
          <p className="mt-4 px-1 text-sm text-ink-soft">
            This is {SITE.name}, with a made-up app. Sort the keywords, switch to the UK, or open Localization and edit the
            keyword field to see which searches it covers.
          </p>
        </div>
      </section>

      <section aria-labelledby="how" className="mx-auto max-w-6xl px-4 pt-28 pb-20 sm:px-6">
        <h2 id="how" className="max-w-[22ch] text-3xl font-bold tracking-tight sm:text-[2.6rem] sm:leading-[1.1]">
          How keyword research works in {SITE.name}
        </h2>
        <ol className="mt-12 grid gap-x-10 gap-y-12 md:grid-cols-2 lg:grid-cols-4">
          {STEPS.map((step, i) => (
            <li key={step.title} className="border-t-2 border-ink pt-5">
              <span className="text-sm font-semibold text-ink-soft">Step {i + 1}</span>
              <h3 className="mt-1 text-xl font-semibold">{step.title}</h3>
              <p className="mt-3 text-ink-soft">{step.body}</p>
              <button type="button" onClick={() => show(step.view)} className="mt-4 text-sm font-semibold underline underline-offset-4 hover:no-underline">
                Show it in the app
              </button>
            </li>
          ))}
        </ol>
      </section>

      <section aria-labelledby="more" className="border-t border-line bg-surface">
        <div className="mx-auto grid max-w-6xl gap-12 px-4 py-20 sm:px-6 lg:grid-cols-[1fr_2fr]">
          <div>
            <h2 id="more" className="text-3xl font-bold tracking-tight">
              The rest of your product page, too
            </h2>
            <p className="mt-4 text-ink-soft">Keywords get people to your page. The rest of {SITE.name} helps them tap Get.</p>
            <button type="button" onClick={() => show("audit")} className={`${buttonClass("secondary", "sm")} mt-6`}>
              See an audit
            </button>
          </div>
          <dl className="grid gap-x-10 gap-y-9 sm:grid-cols-2">
            {rest.map((f) => (
              <div key={f.id}>
                <dt className="text-lg font-semibold">{f.title}</dt>
                <dd className="mt-2 text-ink-soft">{f.body}</dd>
              </div>
            ))}
            <div>
              <dt className="text-lg font-semibold">Every country you sell in</dt>
              <dd className="mt-2 text-ink-soft">
                Track the same app in the US, the UK or anywhere else, and see which countries have no listing in their
                language yet.
              </dd>
            </div>
          </dl>
        </div>
      </section>

      <section aria-labelledby="privacy" className="mx-auto grid max-w-6xl gap-12 px-4 py-20 sm:px-6 lg:grid-cols-2">
        <div>
          <h2 id="privacy" className="text-3xl font-bold tracking-tight">
            Your keywords stay on your computer
          </h2>
          <ul className="mt-6 flex flex-col gap-3 text-ink-soft">
            {PRIVACY_POINTS.map((p) => (
              <li key={p} className="flex gap-3">
                <svg viewBox="0 0 16 16" className="mt-1 size-4 shrink-0 text-up" aria-hidden="true">
                  <path d="m3 8.5 3 3 7-7" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
                </svg>
                {p}
              </li>
            ))}
          </ul>
        </div>
        <DataFlow />
      </section>

      <section aria-labelledby="learn" className="border-t border-line">
        <div className="mx-auto max-w-6xl px-4 py-20 sm:px-6">
          <h2 id="learn" className="text-3xl font-bold tracking-tight">
            Learn ASO keyword research
          </h2>
          <ul className="mt-8 grid gap-6 md:grid-cols-2">
            {GUIDES.map((g) => (
              <li key={g.path}>
                <Link to={g.path} className="group block border-t-2 border-ink pt-5">
                  <span className="text-xl font-semibold group-hover:underline group-hover:underline-offset-4">{g.h1}</span>
                  <span className="mt-2 block max-w-[52ch] text-ink-soft">{g.description}</span>
                </Link>
              </li>
            ))}
          </ul>
        </div>
      </section>

      <section aria-labelledby="questions" className="border-t border-line bg-surface">
        <div className="mx-auto max-w-3xl px-4 py-20 sm:px-6">
          <h2 id="questions" className="text-3xl font-bold tracking-tight">
            Questions
          </h2>
          <div className="mt-8 divide-y divide-line border-y border-line">
            {FAQ.map((f) => (
              <details key={f.q} className="group py-4">
                <summary className="flex cursor-pointer list-none items-center justify-between gap-4 font-semibold">
                  {f.q}
                  <span className="text-ink-soft transition-transform group-open:rotate-45" aria-hidden="true">
                    +
                  </span>
                </summary>
                <p className="mt-3 text-ink-soft">{f.a}</p>
              </details>
            ))}
          </div>
        </div>
      </section>
    </>
  )
}

/** Who talks to whom: the app talks to Apple; our server only handles your account. */
function DataFlow() {
  return (
    <figure className="rounded-2xl border border-line bg-surface p-6">
      <svg viewBox="0 0 420 220" className="w-full" role="img" aria-labelledby="flow-title">
        <title id="flow-title">
          Your computer talks to Apple for keywords, rankings and your listing, and to the AI you choose; the AppDoctor
          server only handles your account and subscription.
        </title>
        <g fontFamily="inherit" fontSize="13" fill="var(--color-ink)">
          <rect x="140" y="80" width="140" height="60" rx="10" fill="var(--color-tint)" stroke="var(--color-up)" />
          <text x="210" y="106" textAnchor="middle" fontWeight="600">
            Your computer
          </text>
          <text x="210" y="124" textAnchor="middle" fontSize="11" fill="var(--color-ink-soft)">
            keywords, ranks, keys
          </text>

          <rect x="10" y="10" width="120" height="50" rx="10" fill="none" stroke="var(--color-line)" />
          <text x="70" y="40" textAnchor="middle">
            App Store
          </text>
          <rect x="290" y="10" width="120" height="50" rx="10" fill="none" stroke="var(--color-line)" />
          <text x="350" y="40" textAnchor="middle">
            Apple Ads
          </text>
          <rect x="10" y="160" width="120" height="50" rx="10" fill="none" stroke="var(--color-line)" />
          <text x="70" y="190" textAnchor="middle">
            App Store Connect
          </text>
          <rect x="290" y="160" width="120" height="50" rx="10" fill="none" stroke="var(--color-line)" strokeDasharray="4 3" />
          <text x="350" y="183" textAnchor="middle">
            {SITE.name}
          </text>
          <text x="350" y="199" textAnchor="middle" fontSize="11" fill="var(--color-ink-soft)">
            sign-in and billing
          </text>
        </g>
        <g stroke="var(--color-ink)" strokeWidth="1.5" fill="none">
          <path d="M150 85 112 60" />
          <path d="M270 85 308 60" />
          <path d="M150 135 112 160" />
        </g>
        <path d="M270 135 308 160" stroke="var(--color-ink-soft)" strokeWidth="1.5" strokeDasharray="4 3" fill="none" />
      </svg>
    </figure>
  )
}
