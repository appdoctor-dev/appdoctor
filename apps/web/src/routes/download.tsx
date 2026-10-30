import { Link, createFileRoute } from "@tanstack/react-router"
import { PageIntro, buttonClass } from "#/components/chrome"
import { SITE } from "#/content/site"
import { getLatestRelease } from "#/lib/data.functions"

export const Route = createFileRoute("/download")({
  loader: () => getLatestRelease(),
  head: () => ({
    meta: [
      { title: `Download ${SITE.name} for Mac and Windows` },
      { name: "description", content: `Download the ${SITE.name} ASO tool for macOS and Windows.` },
    ],
  }),
  component: Download,
})

const TARGETS = [
  { key: "mac", label: "Mac", hint: "Apple silicon and Intel, macOS 13 or later", file: ".dmg" },
  { key: "windows", label: "Windows", hint: "Windows 10 or 11, 64-bit", file: ".zip" },
] as const

function Download() {
  const release = Route.useLoaderData()

  return (
    <>
      <PageIntro title={`Download ${SITE.name}`}>
        Install it, open it and choose Sign in. You need a{" "}
        <Link to="/pricing" className="underline underline-offset-4">
          subscription
        </Link>{" "}
        to use the signed app. The Mac app keeps itself up to date.
      </PageIntro>
      <div className="mx-auto max-w-3xl px-4 pb-20 sm:px-6">
        <ul className="grid gap-4 sm:grid-cols-2">
          {TARGETS.map((t) => (
            <li key={t.key} className="flex flex-col rounded-2xl border border-line bg-surface p-6">
              <h2 className="text-lg font-semibold">{t.label}</h2>
              <p className="text-sm text-ink-soft">{t.hint}</p>
              {release ? (
                <>
                  <p className="mt-4 text-sm text-ink-soft">
                    Version {release.version}, {t.file}
                  </p>
                  <a href={release[t.key]} className={`${buttonClass("primary")} mt-4`}>
                    Download for {t.label}
                  </a>
                </>
              ) : (
                <p className="mt-4 text-sm text-ink-soft">Not available yet. The first release is on its way.</p>
              )}
            </li>
          ))}
        </ul>
        {release && (
          <p className="mt-6 text-sm text-ink-soft">
            <a href={release.notes} className="underline underline-offset-4">
              What's new in {release.version}
            </a>
            {" · "}
            <a href={`${SITE.sourceUrl}/releases`} className="underline underline-offset-4">
              All releases
            </a>
          </p>
        )}
        <p className="mt-10 max-w-2xl text-ink-soft">
          Prefer to build it yourself? The full source is on{" "}
          <a href={SITE.sourceUrl} className="underline underline-offset-4">
            GitHub
          </a>
          , with build instructions for both platforms.
        </p>
      </div>
    </>
  )
}
