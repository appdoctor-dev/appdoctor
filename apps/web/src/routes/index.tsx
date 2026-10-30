import { Link, createFileRoute } from "@tanstack/react-router"
import { AppWindow, type View } from "#/components/app-window"
import { Mark, buttonClass } from "#/components/chrome"
import { KEYWORD_RESEARCH } from "#/content/guides"
import { FAQ, SITE } from "#/content/site"
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

/** One job per section, each shown on a working copy of that screen. */
const SECTIONS: { id: string; title: string; body: string; more: { to: string; label: string }; start: View; ai?: boolean }[] = [
  {
    id: "listing",
    title: "Write the listing search can read.",
    body: "Edit your name, subtitle and keyword field with live counts. AppDoctor shows which tracked keywords your listing can't match yet and which words you've spent twice, then saves to App Store Connect when you confirm.",
    more: { to: "/aso-keyword-research", label: "Read the keyword research guide" },
    start: "localization",
  },
  {
    id: "audit",
    title: "A checkup for your whole product page.",
    body: "One score out of 100 for your listing text, keywords, screenshots and video, ratings and updates, and countries and languages. Then the fixes, worst first.",
    more: { to: "/docs/aso-audit", label: "How the audit works" },
    start: "audit",
  },
  {
    id: "ai",
    title: "Your own AI, already inside.",
    body: "Claude Code, Codex, Gemini CLI or a model in Ollama, with the sign-in you already have. It reads your keywords, ranks and listing and suggests changes. Nothing reaches the App Store until you review it.",
    more: { to: "/docs/ai-agents", label: "Using AppDoctor with AI" },
    start: "keywords",
    ai: true,
  },
  {
    id: "countries",
    title: "Every country, its own keywords.",
    body: "Track the same app in the US, the UK or any storefront. See what climbed after a release, what fell, and which countries have no listing in their language yet.",
    more: { to: "/docs/countries-and-languages", label: "Countries and languages" },
    start: "rankings",
  },
  {
    id: "screenshots",
    title: "Screenshots and previews that sell.",
    body: "Real frames for every iPhone and iPad, the new product page header and search art, and app preview videos exported at Apple's sizes.",
    more: { to: "/docs/screenshots-and-videos", label: "Screenshots and videos" },
    start: "screenshots",
  },
]

function Downloads({ size = "lg" }: { size?: "md" | "lg" }) {
  return (
    <div className="flex flex-wrap justify-center gap-3">
      <Link to="/download" className={buttonClass("primary", size)}>
        Download for Mac
      </Link>
      <Link to="/download" className={buttonClass("secondary", size)}>
        Download for Windows
      </Link>
    </div>
  )
}

function Home() {
  return (
    <>
      <section className="px-5 pt-16 text-center sm:pt-24">
        <Mark className="mx-auto size-[76px] sm:size-[92px]" />
        <h1 className="mx-auto mt-8 max-w-[20ch] text-[2.75rem] leading-[1.05] font-semibold tracking-[-0.035em] text-balance sm:text-[4.6rem]">
          ASO keyword research. With Apple's own numbers.
        </h1>
        <p className="mx-auto mt-6 max-w-[38ch] text-[19px] leading-[1.45] text-balance text-ink-soft sm:text-[22px]">
          Find the App Store searches worth winning, track your rank in every country, and let your own AI rewrite your
          listing.
        </p>
        <div className="mt-9">
          <Downloads />
        </div>
        <p className="mt-5 text-[13px] text-ink-soft">For macOS and Windows. Your keywords stay on your computer.</p>
        <div className="mx-auto mt-16 max-w-[1200px] rounded-[28px] bg-surface p-[clamp(10px,2.6vw,36px)] sm:rounded-[40px]">
          <AppWindow start="keywords" label="AppDoctor's keyword research, with example data" />
        </div>
        <p className="mx-auto mt-4 max-w-2xl text-[13px] text-ink-soft">
          This is the app, with a made-up one. Sort the columns, switch to the UK, add a keyword.
        </p>
      </section>

      {SECTIONS.map((section, i) => (
        <section key={section.id} aria-labelledby={section.id} className={`px-5 py-24 text-center sm:py-32 ${i % 2 === 0 ? "mt-24 bg-surface sm:mt-32" : ""}`}>
          <h2 id={section.id} className="mx-auto max-w-[18ch] text-[2.3rem] leading-[1.08] font-semibold tracking-[-0.03em] text-balance sm:text-[3.5rem]">
            {section.title}
          </h2>
          <p className="mx-auto mt-5 max-w-[46ch] text-[18px] leading-[1.5] text-pretty text-ink-soft sm:text-[21px]">{section.body}</p>
          <Link to={section.more.to} className={`${buttonClass("plain", "md")} mt-4`}>
            {section.more.label}
          </Link>
          <div className={`mx-auto mt-14 max-w-[1100px] rounded-[28px] p-[clamp(8px,2vw,28px)] sm:rounded-[36px] ${i % 2 === 0 ? "bg-white" : "bg-surface"}`}>
            <AppWindow start={section.start} ai={section.ai} height={540} />
          </div>
        </section>
      ))}

      <section aria-labelledby="privacy" className="mt-24 bg-black px-5 py-24 text-center text-white sm:py-32">
        <h2 id="privacy" className="text-[2.3rem] leading-[1.08] font-semibold tracking-[-0.03em] sm:text-[3.5rem]">
          Private by design.
        </h2>
        <p className="mx-auto mt-5 max-w-[44ch] text-[18px] leading-[1.5] text-white/70 sm:text-[21px]">
          Your keywords, ranks and App Store Connect key live on your computer. Our server knows your email and your
          subscription, and nothing else.
        </p>
        <dl className="mx-auto mt-16 grid max-w-[980px] gap-10 text-left sm:grid-cols-3">
          {[
            ["On your computer", "Everything AppDoctor learns about your apps is stored on your Mac or PC, and keys go in its keychain."],
            ["Straight to Apple", "Searches, rank checks and listing changes go from your computer to Apple directly."],
            ["Open source", "Read the code to check all of this, or build the app yourself for free."],
          ].map(([title, body]) => (
            <div key={title} className="border-t border-white/20 pt-5">
              <dt className="text-[19px] font-semibold">{title}</dt>
              <dd className="mt-2 text-white/65">{body}</dd>
            </div>
          ))}
        </dl>
      </section>

      <section aria-label="Learn more" className="mx-auto grid max-w-[1100px] gap-5 px-5 py-24 md:grid-cols-2">
        <Link to={KEYWORD_RESEARCH.path} className="group rounded-[28px] bg-surface p-10 transition-colors hover:bg-[#ececf0]">
          <span className="block text-[13px] font-medium text-ink-soft">Guide</span>
          <span className="mt-2 block text-[28px] leading-tight font-semibold tracking-[-0.02em]">{KEYWORD_RESEARCH.h1}</span>
          <span className="mt-3 block text-ink-soft">{KEYWORD_RESEARCH.description}</span>
          <span className="mt-5 block text-accent group-hover:underline">Read the guide</span>
        </Link>
        <Link to="/docs" className="group rounded-[28px] bg-surface p-10 transition-colors hover:bg-[#ececf0]">
          <span className="block text-[13px] font-medium text-ink-soft">Docs</span>
          <span className="mt-2 block text-[28px] leading-tight font-semibold tracking-[-0.02em]">Set up {SITE.name}</span>
          <span className="mt-3 block text-ink-soft">
            Install it, add your app, connect App Store Connect and Apple Ads, and bring your own AI.
          </span>
          <span className="mt-5 block text-accent group-hover:underline">Read the docs</span>
        </Link>
      </section>

      <section aria-labelledby="questions" className="mx-auto max-w-[760px] px-5 pb-24">
        <h2 id="questions" className="text-center text-[2.3rem] font-semibold tracking-[-0.03em] sm:text-[2.8rem]">
          Questions
        </h2>
        <div className="mt-10 divide-y divide-line border-y border-line">
          {FAQ.map((f) => (
            <details key={f.q} className="group py-5">
              <summary className="flex cursor-pointer list-none items-center justify-between gap-4 text-[17px] font-semibold">
                {f.q}
                <span className="text-[22px] leading-none font-light text-ink-soft transition-transform group-open:rotate-45" aria-hidden="true">
                  +
                </span>
              </summary>
              <p className="mt-3 text-[17px] leading-relaxed text-ink-soft">{f.a}</p>
            </details>
          ))}
        </div>
      </section>

      <section className="bg-surface px-5 py-24 text-center">
        <h2 className="text-[2.3rem] font-semibold tracking-[-0.03em] sm:text-[3.2rem]">Start with your own app.</h2>
        <p className="mx-auto mt-4 max-w-[40ch] text-[19px] text-ink-soft">Paste its App Store link and check where it ranks today.</p>
        <div className="mt-8">
          <Downloads />
        </div>
      </section>
    </>
  )
}
