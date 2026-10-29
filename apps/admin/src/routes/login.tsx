import { createFileRoute } from "@tanstack/react-router"
import { MailCheckIcon } from "lucide-react"
import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { authClient } from "@/lib/api"

export const Route = createFileRoute("/login")({ ssr: false, component: Login })

function Login() {
  const [email, setEmail] = useState("")
  const [state, setState] = useState<"idle" | "sending" | "sent">("idle")
  const [error, setError] = useState<string | null>(null)

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setState("sending")
    setError(null)
    const { error } = await authClient.signIn.magicLink({ email, callbackURL: `${window.location.origin}/` })
    if (error) {
      setError(error.message ?? "Could not send the sign-in link")
      setState("idle")
    } else {
      setState("sent")
    }
  }

  return (
    <main className="flex min-h-svh items-center justify-center p-4">
      <Card className="w-full max-w-sm">
        <CardHeader>
          <CardTitle>AppDoctor Admin</CardTitle>
          <CardDescription>Sign in with an admin email address.</CardDescription>
        </CardHeader>
        <CardContent>
          {state === "sent" ? (
            <div className="flex flex-col items-center gap-3 py-4 text-center">
              <MailCheckIcon className="size-8 text-muted-foreground" />
              <p className="text-sm">
                Check <strong>{email}</strong> for a sign-in link.
              </p>
              <p className="text-xs text-muted-foreground">Running locally? The link is printed in the API console.</p>
            </div>
          ) : (
            <form onSubmit={submit} className="flex flex-col gap-4">
              <div className="flex flex-col gap-2">
                <Label htmlFor="email">Email</Label>
                <Input
                  id="email"
                  type="email"
                  autoComplete="email"
                  required
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                />
              </div>
              {error && <p className="text-sm text-destructive">{error}</p>}
              <Button type="submit" disabled={state === "sending"}>
                {state === "sending" ? "Sending…" : "Email me a sign-in link"}
              </Button>
            </form>
          )}
        </CardContent>
      </Card>
    </main>
  )
}
