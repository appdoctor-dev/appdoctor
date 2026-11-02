import { useQuery } from "@tanstack/react-query"
import { createFileRoute } from "@tanstack/react-router"
import { ExternalLinkIcon } from "lucide-react"
import { ErrorNote, PageBody, PageHeader } from "@/components/page"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { date } from "@/lib/format"

export const Route = createFileRoute("/_admin/releases")({ component: Releases })

/**
 * Releases are built and published on GitHub by .github/workflows/release.yml;
 * the website and the app's updater download from there. This page reads them
 * from GitHub's public API in the browser, for the download counts.
 */
const REPO = "appdoctor-dev/appdoctor"
const MAC = "AppDoctor-macOS.dmg"
const WINDOWS = "AppDoctor-Windows-x64.zip"
const MANIFEST = "latest.json"

type GitHubRelease = {
  id: number
  tag_name: string
  html_url: string
  draft: boolean
  prerelease: boolean
  published_at: string | null
  assets: { name: string; download_count: number }[]
}

async function fetchReleases() {
  const res = await fetch(`https://api.github.com/repos/${REPO}/releases?per_page=30`, {
    headers: { accept: "application/vnd.github+json" },
  })
  if (!res.ok) throw new Error(`GitHub answered ${res.status}. Try again in a minute.`)
  return (await res.json()) as GitHubRelease[]
}

const downloads = (release: GitHubRelease, name: string) =>
  release.assets.find((a) => a.name === name)?.download_count ?? 0

function Releases() {
  const releases = useQuery({ queryKey: ["github-releases"], queryFn: fetchReleases })
  const published = releases.data?.filter((r) => !r.draft) ?? []
  const total = (name: string) => published.reduce((n, r) => n + downloads(r, name), 0)
  const tiles = [
    { label: "Latest", value: published[0]?.tag_name ?? "—", hint: published[0] && `Published ${date(published[0].published_at)}` },
    { label: "Mac downloads", value: total(MAC).toLocaleString(), hint: "All releases, including automatic updates" },
    { label: "Windows downloads", value: total(WINDOWS).toLocaleString(), hint: "All releases" },
  ]

  return (
    <>
      <PageHeader
        title="Releases"
        actions={
          <Button size="sm" asChild>
            <a href={`https://github.com/${REPO}/actions/workflows/release.yml`} target="_blank" rel="noreferrer">
              New release <ExternalLinkIcon />
            </a>
          </Button>
        }
      />
      <PageBody>
        <p className="max-w-2xl text-sm text-muted-foreground">
          Releases are built, signed and published on GitHub. To ship one, open the Release workflow and choose Run
          workflow with patch, minor or major: it raises the version for you.
        </p>
        <ErrorNote error={releases.error} />
        <div className="grid gap-4 sm:grid-cols-3">
          {releases.isPending
            ? Array.from({ length: 3 }, (_, i) => <Skeleton key={i} className="h-28" />)
            : tiles.map((t) => (
                <Card key={t.label}>
                  <CardHeader>
                    <CardDescription>{t.label}</CardDescription>
                    <CardTitle className="text-3xl tabular-nums">{t.value}</CardTitle>
                    {t.hint && <CardDescription className="text-xs">{t.hint}</CardDescription>}
                  </CardHeader>
                </Card>
              ))}
        </div>
        <div className="rounded-lg border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Version</TableHead>
                <TableHead>Published</TableHead>
                <TableHead className="text-right">Mac</TableHead>
                <TableHead className="text-right">Windows</TableHead>
                <TableHead className="text-right" title="How often installed apps fetched this release's latest.json">
                  Update checks
                </TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {releases.data?.map((r) => (
                <TableRow key={r.id}>
                  <TableCell className="font-medium">
                    {r.tag_name}
                    {r.draft && (
                      <Badge variant="outline" className="ml-2">
                        draft
                      </Badge>
                    )}
                    {r.prerelease && (
                      <Badge variant="secondary" className="ml-2">
                        pre-release
                      </Badge>
                    )}
                  </TableCell>
                  <TableCell>{date(r.published_at)}</TableCell>
                  <TableCell className="text-right tabular-nums">{downloads(r, MAC).toLocaleString()}</TableCell>
                  <TableCell className="text-right tabular-nums">{downloads(r, WINDOWS).toLocaleString()}</TableCell>
                  <TableCell className="text-right tabular-nums">{downloads(r, MANIFEST).toLocaleString()}</TableCell>
                  <TableCell className="text-right">
                    <a
                      href={r.html_url}
                      target="_blank"
                      rel="noreferrer"
                      className="text-sm text-muted-foreground underline-offset-4 hover:underline"
                    >
                      GitHub
                    </a>
                  </TableCell>
                </TableRow>
              ))}
              {releases.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6} className="py-10 text-center text-muted-foreground">
                    No releases yet.
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </div>
      </PageBody>
    </>
  )
}
