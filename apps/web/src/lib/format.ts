export function money(minor: number | null | undefined, currency = "usd") {
  if (minor == null) return "—"
  const format = new Intl.NumberFormat("en", { style: "currency", currency: currency.toUpperCase() })
  const digits = format.resolvedOptions().maximumFractionDigits ?? 2
  const value = minor / 10 ** digits
  // Whole amounts read better without ".00" on a pricing page.
  return Number.isInteger(value)
    ? new Intl.NumberFormat("en", { style: "currency", currency: currency.toUpperCase(), maximumFractionDigits: 0 }).format(value)
    : format.format(value)
}

export function date(value: string | number | Date | null | undefined) {
  if (value == null) return ""
  return new Intl.DateTimeFormat("en", { dateStyle: "long", timeZone: "UTC" }).format(new Date(value))
}

export function bytes(n: number) {
  return `${(n / 1024 / 1024).toFixed(0)} MB`
}
