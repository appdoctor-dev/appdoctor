import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { createFileRoute } from "@tanstack/react-router"
import { useState } from "react"
import { toast } from "sonner"
import { ErrorNote, PageBody, PageHeader } from "@/components/page"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Progress } from "@/components/ui/progress"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Switch } from "@/components/ui/switch"
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table"
import { Textarea } from "@/components/ui/textarea"
import { API_URL, ApiError, adminApi, call } from "@/lib/api"
import { bytes, date } from "@/lib/format"

export const Route = createFileRoute("/_admin/releases")({ component: Releases })

// R2 multipart parts must be at least 5 MB (except the last); 25 MB keeps each
// request well under the Workers request body limit.
const PART_SIZE = 25 * 1024 * 1024

type Platform = "macos" | "windows"
type Arch = "arm64" | "x64" | "universal"

function Releases() {
  const queryClient = useQueryClient()
  const releases = useQuery({ queryKey: ["releases"], queryFn: () => call(adminApi.releases.$get()) })
  const update = useMutation({
    mutationFn: ({ id, published }: { id: string; published: boolean }) =>
      call(adminApi.releases[":id"].$patch({ param: { id }, json: { published } })),
    onSuccess: (r) => {
      toast.success(r.published ? `${r.version} is live` : `${r.version} unpublished`)
      queryClient.invalidateQueries({ queryKey: ["releases"] })
    },
    onError: (e) => toast.error(e.message),
  })
  const remove = useMutation({
    mutationFn: (id: string) => call(adminApi.releases[":id"].$delete({ param: { id } })),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["releases"] }),
    onError: (e) => toast.error(e.message),
  })

  return (
    <>
      <PageHeader title="Releases" />
      <PageBody>
        <UploadCard onUploaded={() => queryClient.invalidateQueries({ queryKey: ["releases"] })} />
        <ErrorNote error={releases.error} />
        <div className="rounded-lg border">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Version</TableHead>
                <TableHead>Target</TableHead>
                <TableHead>File</TableHead>
                <TableHead>Uploaded</TableHead>
                <TableHead>Published</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {releases.data?.map((r) => (
                <TableRow key={r.id}>
                  <TableCell className="font-medium">{r.version}</TableCell>
                  <TableCell>
                    <Badge variant="secondary">
                      {r.platform} · {r.arch}
                    </Badge>
                  </TableCell>
                  <TableCell>
                    <div className="text-sm">{r.filename}</div>
                    <div className="text-xs text-muted-foreground">
                      {bytes(r.size)}
                      {!r.signature && " · unsigned for updater"}
                    </div>
                  </TableCell>
                  <TableCell>{date(r.createdAt)}</TableCell>
                  <TableCell>
                    <Switch
                      checked={r.published}
                      disabled={update.isPending}
                      onCheckedChange={(published) => update.mutate({ id: r.id, published })}
                    />
                  </TableCell>
                  <TableCell className="text-right">
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={remove.isPending}
                      onClick={() => {
                        if (confirm(`Delete ${r.filename}? This removes the file from R2.`)) remove.mutate(r.id)
                      }}
                    >
                      Delete
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
              {releases.data?.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6} className="h-24 text-center text-muted-foreground">
                    No builds yet. Upload a signed .dmg or Windows installer above.
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

function UploadCard({ onUploaded }: { onUploaded: () => void }) {
  const [file, setFile] = useState<File | null>(null)
  const [version, setVersion] = useState("")
  const [platform, setPlatform] = useState<Platform>("macos")
  const [arch, setArch] = useState<Arch>("universal")
  const [notes, setNotes] = useState("")
  const [signature, setSignature] = useState("")
  const [progress, setProgress] = useState<number | null>(null)

  const upload = useMutation({
    mutationFn: async () => {
      if (!file) throw new Error("Choose a file")
      const target = { version: version.trim(), platform, arch, filename: file.name }
      const { key, uploadId } = await call(
        adminApi.releases.uploads.$post({ json: { ...target, contentType: file.type || "application/octet-stream" } }),
      )

      const parts: { partNumber: number; etag: string }[] = []
      const total = Math.ceil(file.size / PART_SIZE)
      setProgress(0)
      for (let i = 0; i < total; i++) {
        const partNumber = i + 1
        // Raw fetch: the body is a binary chunk, which the typed JSON client does not model.
        const res = await fetch(
          `${API_URL}/admin/releases/uploads/${encodeURIComponent(uploadId)}/parts/${partNumber}?key=${encodeURIComponent(key)}`,
          { method: "PUT", credentials: "include", body: file.slice(i * PART_SIZE, (i + 1) * PART_SIZE) },
        )
        const body = await res.json().catch(() => null)
        if (!res.ok) throw new ApiError(res.status, body)
        parts.push(body as { partNumber: number; etag: string })
        setProgress(Math.round((partNumber / total) * 100))
      }

      return call(
        adminApi.releases.uploads[":uploadId"].complete.$post({
          param: { uploadId },
          json: { ...target, key, size: file.size, notes, signature: signature.trim() || null, parts },
        }),
      )
    },
    onSuccess: (r) => {
      toast.success(`Uploaded ${r.version} (${r.platform} ${r.arch}). Publish it when ready.`)
      setFile(null)
      setNotes("")
      setSignature("")
      onUploaded()
    },
    onError: (e) => toast.error(e.message),
    onSettled: () => setProgress(null),
  })

  return (
    <Card>
      <CardHeader>
        <CardTitle>Upload a build</CardTitle>
        <CardDescription>
          Stored in R2. Customers download the newest published build for their platform; the desktop app's updater
          checks against it.
        </CardDescription>
      </CardHeader>
      <CardContent className="grid gap-4 md:grid-cols-2">
        <div className="flex flex-col gap-2 md:col-span-2">
          <Label htmlFor="file">Installer</Label>
          <Input id="file" type="file" accept=".dmg,.zip,.exe,.msi,.msix,.gz" onChange={(e) => setFile(e.target.files?.[0] ?? null)} />
        </div>
        <div className="flex flex-col gap-2">
          <Label htmlFor="version">Version</Label>
          <Input id="version" placeholder="1.0.0" value={version} onChange={(e) => setVersion(e.target.value)} />
        </div>
        <div className="grid grid-cols-2 gap-4">
          <div className="flex flex-col gap-2">
            <Label>Platform</Label>
            <Select value={platform} onValueChange={(v) => setPlatform(v as Platform)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="macos">macOS</SelectItem>
                <SelectItem value="windows">Windows</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex flex-col gap-2">
            <Label>Architecture</Label>
            <Select value={arch} onValueChange={(v) => setArch(v as Arch)}>
              <SelectTrigger>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="universal">Universal</SelectItem>
                <SelectItem value="arm64">arm64</SelectItem>
                <SelectItem value="x64">x64</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>
        <div className="flex flex-col gap-2">
          <Label htmlFor="notes">Release notes</Label>
          <Textarea id="notes" rows={3} value={notes} onChange={(e) => setNotes(e.target.value)} />
        </div>
        <div className="flex flex-col gap-2">
          <Label htmlFor="signature">Updater signature (optional)</Label>
          <Textarea id="signature" rows={3} className="font-mono text-xs" value={signature} onChange={(e) => setSignature(e.target.value)} />
        </div>
        <div className="flex items-center gap-4 md:col-span-2">
          <Button onClick={() => upload.mutate()} disabled={upload.isPending || !file || !version}>
            {upload.isPending ? "Uploading…" : "Upload"}
          </Button>
          {progress !== null && <Progress value={progress} className="max-w-xs" />}
        </div>
      </CardContent>
    </Card>
  )
}
