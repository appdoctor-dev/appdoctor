import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"
import { createFileRoute } from "@tanstack/react-router"
import { PlusIcon } from "lucide-react"
import { useState } from "react"
import { toast } from "sonner"
import { ErrorNote, PageBody, PageHeader } from "@/components/page"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from "@/components/ui/card"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Switch } from "@/components/ui/switch"
import { Textarea } from "@/components/ui/textarea"
import { adminApi, call } from "@/lib/api"
import { money, toMajor, toMinor } from "@/lib/format"

export const Route = createFileRoute("/_admin/pricing")({ component: Pricing })

type Plan = Awaited<ReturnType<typeof fetchPlans>>[number]
const fetchPlans = () => call(adminApi.plans.$get())

function usePlanMutation<T>(fn: (input: T) => Promise<unknown>, success: string, onDone?: () => void) {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: fn,
    onSuccess: () => {
      toast.success(success)
      queryClient.invalidateQueries({ queryKey: ["plans"] })
      onDone?.()
    },
    onError: (e) => toast.error(e.message),
  })
}

function Pricing() {
  const plans = useQuery({ queryKey: ["plans"], queryFn: fetchPlans })

  return (
    <>
      <PageHeader title="Pricing" actions={<NewPlanDialog />} />
      <PageBody>
        <p className="max-w-2xl text-sm text-muted-foreground">
          Each plan is a Stripe product. Changing a price creates a new Stripe price and archives the old one:
          new customers pay the new price, existing subscribers keep the price they signed up at.
        </p>
        <ErrorNote error={plans.error} />
        {plans.data?.length === 0 && (
          <Card>
            <CardContent className="py-10 text-center text-sm text-muted-foreground">
              No plans yet. Create one to start selling.
            </CardContent>
          </Card>
        )}
        <div className="grid gap-4 lg:grid-cols-2 2xl:grid-cols-3">
          {plans.data?.map((plan) => <PlanCard key={plan.id} plan={plan} />)}
        </div>
      </PageBody>
    </>
  )
}

function PlanCard({ plan }: { plan: Plan }) {
  const toggle = usePlanMutation(
    (active: boolean) => call(adminApi.plans[":id"].$patch({ param: { id: plan.id }, json: { active } })),
    plan.active ? "Plan hidden from new customers" : "Plan is live",
  )

  return (
    <Card className={plan.active ? undefined : "opacity-70"}>
      <CardHeader>
        <div className="flex items-start justify-between gap-2">
          <div>
            <CardTitle>{plan.name}</CardTitle>
            <CardDescription className="font-mono text-xs">{plan.id}</CardDescription>
          </div>
          <div className="flex items-center gap-2 text-sm">
            <Label htmlFor={`active-${plan.id}`}>{plan.active ? "Live" : "Hidden"}</Label>
            <Switch
              id={`active-${plan.id}`}
              checked={plan.active}
              disabled={toggle.isPending}
              onCheckedChange={(v) => toggle.mutate(v)}
            />
          </div>
        </div>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 text-sm">
        <div className="flex flex-wrap gap-x-6 gap-y-1">
          <span>
            <span className="text-2xl font-semibold tabular-nums">{money(plan.monthlyAmount, plan.currency)}</span>
            <span className="text-muted-foreground"> / month</span>
          </span>
          {plan.yearlyAmount && (
            <span className="self-end text-muted-foreground">{money(plan.yearlyAmount, plan.currency)} / year</span>
          )}
        </div>
        {plan.description && <p className="text-muted-foreground">{plan.description}</p>}
        <div className="flex flex-wrap gap-2">
          <Badge variant="secondary">{plan.maxDevices} devices</Badge>
          {plan.trialDays > 0 && <Badge variant="secondary">{plan.trialDays}-day trial</Badge>}
        </div>
        {plan.features.length > 0 && (
          <ul className="list-disc pl-5 text-muted-foreground">
            {plan.features.map((f) => (
              <li key={f}>{f}</li>
            ))}
          </ul>
        )}
      </CardContent>
      <CardFooter className="gap-2">
        <PriceDialog plan={plan} />
        <EditPlanDialog plan={plan} />
      </CardFooter>
    </Card>
  )
}

function PriceDialog({ plan }: { plan: Plan }) {
  const [open, setOpen] = useState(false)
  const [monthly, setMonthly] = useState(toMajor(plan.monthlyAmount, plan.currency))
  const [yearly, setYearly] = useState(toMajor(plan.yearlyAmount, plan.currency))
  const update = usePlanMutation(
    async () => {
      const nextMonthly = toMinor(monthly, plan.currency)
      const nextYearly = yearly.trim() ? toMinor(yearly, plan.currency) : null
      if (!nextMonthly) throw new Error("Enter a monthly price")
      if (yearly.trim() && !nextYearly) throw new Error("Enter a valid yearly price, or leave it empty")
      const id = { param: { id: plan.id } }
      if (nextMonthly !== plan.monthlyAmount) {
        await call(adminApi.plans[":id"].prices.$post({ ...id, json: { interval: "month", amount: nextMonthly } }))
      }
      if (nextYearly !== (plan.yearlyPriceId ? plan.yearlyAmount : null)) {
        await call(adminApi.plans[":id"].prices.$post({ ...id, json: { interval: "year", amount: nextYearly } }))
      }
    },
    "Prices updated",
    () => setOpen(false),
  )

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button size="sm">Change price</Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Change {plan.name} pricing</DialogTitle>
          <DialogDescription>Amounts in {plan.currency.toUpperCase()}.</DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-4">
          <div className="flex flex-col gap-2">
            <Label htmlFor="monthly">Monthly</Label>
            <Input id="monthly" inputMode="decimal" value={monthly} onChange={(e) => setMonthly(e.target.value)} />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="yearly">Yearly (optional)</Label>
            <Input id="yearly" inputMode="decimal" value={yearly} onChange={(e) => setYearly(e.target.value)} />
          </div>
        </div>
        <Alert>
          <AlertDescription>
            Existing subscribers keep their current price. To move them, change their subscription in Stripe.
          </AlertDescription>
        </Alert>
        <DialogFooter>
          <Button onClick={() => update.mutate(undefined)} disabled={update.isPending}>
            {update.isPending ? "Saving…" : "Save prices"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

type DetailsForm = { name: string; description: string; features: string; maxDevices: string; trialDays: string; sort: string }

function DetailsFields({ form, set }: { form: DetailsForm; set: (patch: Partial<DetailsForm>) => void }) {
  return (
    <>
      <div className="flex flex-col gap-2">
        <Label htmlFor="name">Name</Label>
        <Input id="name" value={form.name} onChange={(e) => set({ name: e.target.value })} />
      </div>
      <div className="flex flex-col gap-2">
        <Label htmlFor="description">Description</Label>
        <Input id="description" value={form.description} onChange={(e) => set({ description: e.target.value })} />
      </div>
      <div className="flex flex-col gap-2">
        <Label htmlFor="features">Features (one per line)</Label>
        <Textarea id="features" rows={4} value={form.features} onChange={(e) => set({ features: e.target.value })} />
      </div>
      <div className="grid grid-cols-3 gap-4">
        <div className="flex flex-col gap-2">
          <Label htmlFor="devices">Devices</Label>
          <Input id="devices" type="number" min={1} value={form.maxDevices} onChange={(e) => set({ maxDevices: e.target.value })} />
        </div>
        <div className="flex flex-col gap-2">
          <Label htmlFor="trial">Trial days</Label>
          <Input id="trial" type="number" min={0} value={form.trialDays} onChange={(e) => set({ trialDays: e.target.value })} />
        </div>
        <div className="flex flex-col gap-2">
          <Label htmlFor="sort">Order</Label>
          <Input id="sort" type="number" value={form.sort} onChange={(e) => set({ sort: e.target.value })} />
        </div>
      </div>
    </>
  )
}

function detailsPayload(form: DetailsForm) {
  return {
    name: form.name.trim(),
    description: form.description.trim(),
    features: form.features.split("\n").map((f) => f.trim()).filter(Boolean),
    maxDevices: Number(form.maxDevices),
    trialDays: Number(form.trialDays),
    sort: Number(form.sort),
  }
}

function EditPlanDialog({ plan }: { plan: Plan }) {
  const [open, setOpen] = useState(false)
  const [form, setForm] = useState<DetailsForm>({
    name: plan.name,
    description: plan.description,
    features: plan.features.join("\n"),
    maxDevices: String(plan.maxDevices),
    trialDays: String(plan.trialDays),
    sort: String(plan.sort),
  })
  const save = usePlanMutation(
    () => call(adminApi.plans[":id"].$patch({ param: { id: plan.id }, json: detailsPayload(form) })),
    "Plan saved",
    () => setOpen(false),
  )

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button size="sm" variant="outline">
          Edit details
        </Button>
      </DialogTrigger>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Edit {plan.name}</DialogTitle>
        </DialogHeader>
        <DetailsFields form={form} set={(patch) => setForm((f) => ({ ...f, ...patch }))} />
        <DialogFooter>
          <Button onClick={() => save.mutate(undefined)} disabled={save.isPending}>
            {save.isPending ? "Saving…" : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

function NewPlanDialog() {
  const [open, setOpen] = useState(false)
  const [id, setId] = useState("")
  const [currency, setCurrency] = useState("usd")
  const [monthly, setMonthly] = useState("")
  const [yearly, setYearly] = useState("")
  const [form, setForm] = useState<DetailsForm>({
    name: "",
    description: "",
    features: "",
    maxDevices: "2",
    trialDays: "7",
    sort: "0",
  })
  const create = usePlanMutation(
    () => {
      const monthlyAmount = toMinor(monthly, currency)
      if (!monthlyAmount) throw new Error("Enter a monthly price")
      return call(
        adminApi.plans.$post({
          json: {
            id: id.trim(),
            currency,
            monthlyAmount,
            yearlyAmount: yearly.trim() ? toMinor(yearly, currency) : null,
            ...detailsPayload(form),
          },
        }),
      )
    },
    "Plan created in Stripe",
    () => setOpen(false),
  )

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button size="sm">
          <PlusIcon /> New plan
        </Button>
      </DialogTrigger>
      <DialogContent className="max-h-[90svh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle>New plan</DialogTitle>
          <DialogDescription>Creates a Stripe product with monthly (and optional yearly) prices.</DialogDescription>
        </DialogHeader>
        <div className="grid grid-cols-2 gap-4">
          <div className="flex flex-col gap-2">
            <Label htmlFor="plan-id">ID</Label>
            <Input id="plan-id" placeholder="pro" value={id} onChange={(e) => setId(e.target.value.toLowerCase())} />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="currency">Currency</Label>
            <Input id="currency" maxLength={3} value={currency} onChange={(e) => setCurrency(e.target.value.toLowerCase())} />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="new-monthly">Monthly price</Label>
            <Input id="new-monthly" inputMode="decimal" placeholder="12" value={monthly} onChange={(e) => setMonthly(e.target.value)} />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="new-yearly">Yearly price (optional)</Label>
            <Input id="new-yearly" inputMode="decimal" placeholder="99" value={yearly} onChange={(e) => setYearly(e.target.value)} />
          </div>
        </div>
        <DetailsFields form={form} set={(patch) => setForm((f) => ({ ...f, ...patch }))} />
        <DialogFooter>
          <Button onClick={() => create.mutate(undefined)} disabled={create.isPending || !id || !form.name}>
            {create.isPending ? "Creating…" : "Create plan"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
