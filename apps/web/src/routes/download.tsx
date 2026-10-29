import { Link, createFileRoute } from "@tanstack/react-router"
import { PageIntro, buttonClass } from "#/components/chrome"
import { SITE } from "#/content/site"
import { API_URL } from "#/lib/auth"
import { getLatestBuilds } from "#/lib/data.functions"
import { bytes } from "#/lib/format"

export const Route = createFileRoute("/download")({
  loader: () => getLatestBuilds(),
  head: () => ({
    meta: [
      { title: `Download ${SITE.name} for Mac and Windows` },
      { name: "description", content: `Download the ${SITE.name} ASO tool for macOS and Windows.` },
    ],
  }),
  component: Download,
})

const TARGETS = [
  { platform: "macos", arch: "arm64", label: "Mac with Apple silicon", hint: "M1 or later" },
  { platform: "macos", arch: "x64", label: "Mac with Intel", hint: "" },
  { platform: "windows", arch: "x64", label: "Windows", hint: "Windows 10 or 11, 64-bit" },
] as const

function Download() {
  const builds = Route.useLoaderData()

  return (
    <>
      <PageIntro title={`Download ${SITE.name}`}>
        Install it, open it and choose Sign in. You need a{" "}
        <Link to="/pricing" className="underline underline-offset-4">
          subscription
        </Link>{" "}
        to use the signed app.
      </PageIntro>
      <div className="mx-auto max-w-6xl px-4 pb-20 sm:px-6">
        <ul className="grid gap-4 md:grid-cols-3">
          {TARGETS.map((t) => {
            const build = builds.find((b) => b.platform === t.platform && b.arch === t.arch)
            return (
              <li key={`${t.platform}-${t.arch}`} className="flex flex-col rounded-2xl border border-line bg-surface p-6">
                <h2 className="text-lg font-semibold">{t.label}</h2>
                {t.hint && <p className="text-sm text-ink-soft">{t.hint}</p>}
                {build ? (
                  <>
                    <p className="mt-4 text-sm text-ink-soft">
                      Version {build.version}, {bytes(build.size)}
                    </p>
                    <a href={`${API_URL}/v1/downloads/${t.platform}/${t.arch}`} className={`${buttonClass("primary")} mt-4`}>
                      Download
                    </a>
                  </>
                ) : (
                  <p className="mt-4 text-sm text-ink-soft">Not available yet. The first release is on its way.</p>
                )}
              </li>
            )
          })}
        </ul>
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
