import { useEffect, useState } from "react"

// The hero: what AppDoctor shows you, with example data. One rank improves
// shortly after load, which is the only motion on the page.

type Row = { keyword: string; popularity: number; difficulty: number; ranks: [string, number | null, number][] }

const ROWS: Row[] = [
  { keyword: "meditation timer", popularity: 62, difficulty: 38, ranks: [["US", 4, 3], ["DE", 11, 1], ["JP", 23, -2]] },
  { keyword: "breathing exercises", popularity: 55, difficulty: 44, ranks: [["US", 9, 0], ["DE", 7, 2], ["JP", 31, 4]] },
  { keyword: "sleep sounds", popularity: 71, difficulty: 82, ranks: [["US", 38, -5], ["DE", 52, 0], ["JP", null, 0]] },
  { keyword: "focus timer", popularity: 48, difficulty: 29, ranks: [["US", 6, 8], ["DE", 3, 1], ["JP", 14, 0]] },
]

export function KeywordTable() {
  // Start the first US rank at its previous value and let it move once.
  const [settled, setSettled] = useState(false)
  useEffect(() => {
    const t = setTimeout(() => setSettled(true), 900)
    return () => clearTimeout(t)
  }, [])

  return (
    <figure className="overflow-hidden rounded-2xl border border-line bg-surface shadow-[0_1px_0_var(--color-line),0_24px_48px_-24px_rgb(15_27_45/0.25)]">
      <div className="flex items-center gap-3 border-b border-line px-4 py-3">
        <svg viewBox="0 0 20 20" className="size-4 text-ink-soft" aria-hidden="true">
          <circle cx="8.5" cy="8.5" r="5.5" fill="none" stroke="currentColor" strokeWidth="1.8" />
          <path d="m13 13 4 4" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" />
        </svg>
        <span className="text-sm font-medium">Tidepool: Meditation Timer</span>
        <span className="ml-auto text-xs text-ink-soft">Example data</span>
      </div>
      <table className="w-full text-left text-sm tabular-nums">
        <caption className="sr-only">Example keywords with Apple popularity, difficulty and rank by country</caption>
        <thead className="text-xs text-ink-soft">
          <tr className="border-b border-line">
            <th scope="col" className="px-4 py-2.5 font-medium">
              Keyword
            </th>
            <th scope="col" className="px-2 py-2.5 font-medium">
              Popularity
            </th>
            <th scope="col" className="hidden px-2 py-2.5 font-medium sm:table-cell">
              Difficulty
            </th>
            {ROWS[0]!.ranks.map(([country], i) => (
              <th key={country} scope="col" className={`px-3 py-2.5 text-right font-medium ${i === 2 ? "hidden sm:table-cell" : ""}`}>
                {country}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {ROWS.map((row, rowIndex) => (
            <tr key={row.keyword} className="border-b border-line last:border-0">
              <th scope="row" className="px-4 py-3 font-medium">
                {row.keyword}
              </th>
              <td className="px-2 py-3">
                <Meter value={row.popularity} />
              </td>
              <td className="hidden px-2 py-3 text-ink-soft sm:table-cell">{row.difficulty}</td>
              {row.ranks.map(([country, rank, change], i) => {
                const animated = rowIndex === 0 && i === 0
                const shownRank = animated && !settled ? rank! + change : rank
                const shownChange = animated && !settled ? 0 : change
                return (
                  <td
                    key={country}
                    className={`px-3 py-3 text-right ${animated && settled ? "rank-up" : ""} ${i === 2 ? "hidden sm:table-cell" : ""}`}
                    aria-live={animated ? "polite" : undefined}
                  >
                    <span className="font-semibold">{shownRank ? `#${shownRank}` : "–"}</span>
                    <Change value={shownChange} />
                  </td>
                )
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </figure>
  )
}

function Meter({ value }: { value: number }) {
  return (
    <span className="flex items-center gap-2">
      <span className="w-6 text-right">{value}</span>
      <span className="h-1.5 w-14 overflow-hidden rounded-full bg-line" aria-hidden="true">
        <span className="block h-full rounded-full bg-ink" style={{ width: `${value}%` }} />
      </span>
    </span>
  )
}

function Change({ value }: { value: number }) {
  if (value === 0) return <span className="ml-1.5 inline-block w-7 text-xs text-ink-soft">·</span>
  const up = value > 0
  return (
    <span className={`ml-1.5 inline-block w-7 text-xs font-medium ${up ? "text-up" : "text-down"}`}>
      <span className="sr-only">{up ? "up" : "down"} </span>
      {up ? "▲" : "▼"}
      {Math.abs(value)}
    </span>
  )
}
