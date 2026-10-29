import { type Db, type Release, releases } from "@repo/db";
import { and, eq, inArray } from "drizzle-orm";

export const PLATFORMS = ["macos", "windows"] as const;
export const ARCHES = ["arm64", "x64", "universal"] as const;
export type Platform = (typeof PLATFORMS)[number];
export type Arch = (typeof ARCHES)[number];

/** Newest published build that runs on this platform/arch; universal builds match any arch. */
export async function latestRelease(db: Db, platform: Platform, arch: Arch): Promise<Release | null> {
  const rows = await db
    .select()
    .from(releases)
    .where(
      and(
        eq(releases.published, true),
        eq(releases.platform, platform),
        inArray(releases.arch, arch === "universal" ? ["universal"] : [arch, "universal"]),
      ),
    );
  return rows.sort((a, b) => compareVersions(b.version, a.version))[0] ?? null;
}

/** Compares dotted numeric versions ("1.10.0" > "1.9.3"); pre-release suffixes sort first. */
export function compareVersions(a: string, b: string) {
  const [coreA = "", preA] = a.replace(/^v/, "").split("-", 2);
  const [coreB = "", preB] = b.replace(/^v/, "").split("-", 2);
  const partsA = coreA.split(".").map(Number);
  const partsB = coreB.split(".").map(Number);
  for (let i = 0; i < Math.max(partsA.length, partsB.length); i++) {
    const diff = (partsA[i] ?? 0) - (partsB[i] ?? 0);
    if (diff !== 0) return diff;
  }
  if (preA && !preB) return -1;
  if (!preA && preB) return 1;
  return (preA ?? "").localeCompare(preB ?? "");
}
