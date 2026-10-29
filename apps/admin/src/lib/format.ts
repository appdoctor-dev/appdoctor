export function money(minor: number | null | undefined, currency = "usd") {
  if (minor == null) return "—"
  const digits = currencyDigits(currency)
  return new Intl.NumberFormat(undefined, { style: "currency", currency: currency.toUpperCase() }).format(
    minor / 10 ** digits,
  )
}

/** Major-unit text from a form ("12.99") to Stripe minor units (1299), respecting zero-decimal currencies. */
export function toMinor(major: string, currency = "usd") {
  const value = Number.parseFloat(major)
  if (!Number.isFinite(value) || value <= 0) return null
  return Math.round(value * 10 ** currencyDigits(currency))
}

export function toMajor(minor: number | null | undefined, currency = "usd") {
  return minor == null ? "" : String(minor / 10 ** currencyDigits(currency))
}

function currencyDigits(currency: string) {
  return (
    new Intl.NumberFormat(undefined, { style: "currency", currency: currency.toUpperCase() }).resolvedOptions()
      .maximumFractionDigits ?? 2
  )
}

export function date(value: string | number | Date | null | undefined) {
  if (value == null) return "—"
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value))
}

export function relative(value: string | number | Date | null | undefined) {
  if (value == null) return "—"
  const days = Math.round((new Date(value).getTime() - Date.now()) / 86_400_000)
  return new Intl.RelativeTimeFormat(undefined, { numeric: "auto" }).format(days, "day")
}

export function bytes(n: number) {
  const units = ["B", "KB", "MB", "GB"]
  let i = 0
  while (n >= 1024 && i < units.length - 1) {
    n /= 1024
    i++
  }
  return `${n.toFixed(i === 0 ? 0 : 1)} ${units[i]}`
}

export function stripeDashboardUrl(path: string) {
  // Test-mode objects live under /test; the Dashboard redirects live IDs correctly either way.
  return `https://dashboard.stripe.com/${path}`
}
