import { Link, createFileRoute } from "@tanstack/react-router"
import { buttonClass } from "#/components/chrome"
import { KeywordTable } from "#/components/keyword-table"
import { FAQ, FEATURES, PRIVACY_POINTS, SITE } from "#/content/site"

export const Route = createFileRoute("/")({
  head: () => ({
    scripts: [
      {
        type: "application/ld+json",
        children: JSON.stringify({
          "@context": "https://schema.org",
          "@type": "FAQPage",
          mainEntity: FAQ.map((f) => ({ "@type": "Question", name: f.q, acceptedAnswer: { "@type": "Answer", text: f.a } })),
        }),
      },
    ],
  }),
  component: Home,
})

function Home() {
  const available = FEATURES.filter((f) => !f.soon)
  const coming = FEATURES.filter((f) => f.soon)

  return (
    <>
      <section className="mx-auto grid max-w-6xl items-center gap-12 px-4 pt-16 pb-20 sm:px-6 lg:grid-cols-[1fr_1.15fr] lg:pt-24">
        <div>
          <h1 className="text-[2.6rem] leading-[1.05] font-bold tracking-tight text-balance sm:text-6xl">
            Find the searches that bring people to your app.
          </h1>
          <p className="mt-6 max-w-xl text-lg text-ink-soft">
            {SITE.name} is an ASO tool for keyword research and rank tracking. It shows Apple's own popularity score for
            every keyword, where you rank in each country, and lets you update your listing in App Store Connect. It runs
            on your Mac or PC, and your data stays there.
          </p>
          <div className="mt-8 flex flex-wrap gap-3">
            <Link to="/download" className={buttonClass("primary")}>
              Download for Mac or Windows
            </Link>
            <Link to="/pricing" className={buttonClass("secondary")}>
              See pricing
            </Link>
          </div>
        </div>
        <KeywordTable />
      </section>

      <section id="features" className="border-t border-line bg-surface">
        <div className="mx-auto max-w-6xl px-4 py-20 sm:px-6">
          <h2 className="max-w-2xl text-3xl font-bold tracking-tight sm:text-4xl">
            Keyword research tools that use Apple's numbers, not guesses
          </h2>
          <dl className="mt-12 grid gap-x-12 gap-y-10 md:grid-cols-3">
            {available.map((f) => (
              <div key={f.id}>
                <dt className="text-lg font-semibold">{f.title}</dt>
                <dd className="mt-2 text-ink-soft">{f.body}</dd>
              </div>
            ))}
          </dl>
          <h3 className="mt-16 text-sm font-semibold text-ink-soft">Coming next</h3>
          <dl className="mt-4 grid gap-x-12 gap-y-6 border-t border-line pt-6 md:grid-cols-3">
            {coming.map((f) => (
              <div key={f.id}>
                <dt className="font-semibold">{f.title}</dt>
                <dd className="mt-1.5 text-sm text-ink-soft">{f.body}</dd>
              </div>
            ))}
          </dl>
        </div>
      </section>

      <section className="mx-auto grid max-w-6xl gap-12 px-4 py-20 sm:px-6 lg:grid-cols-2">
        <div>
          <h2 className="text-3xl font-bold tracking-tight">Your keywords stay on your computer</h2>
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

      <section className="border-t border-line bg-surface">
        <div className="mx-auto max-w-3xl px-4 py-20 sm:px-6">
          <h2 className="text-3xl font-bold tracking-tight">Questions</h2>
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
          Your computer talks to Apple for keywords, rankings and your listing; the AppDoctor server only handles your
          account and subscription.
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
