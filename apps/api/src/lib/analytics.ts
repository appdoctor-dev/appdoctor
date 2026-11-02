import { type Db, type NewSiteVisit, siteVisits } from "@repo/db";
import { and, count, desc, gte, inArray, isNotNull, isNull, lt, or, sql } from "drizzle-orm";

const DAY_MS = 86_400_000;
/** Visits older than this are deleted, a little at a time, as new ones arrive. */
const KEEP_DAYS = 400;

/** Called by the web Worker (through SiteEntrypoint) for each landing or agent read. */
export async function recordVisit(db: Db, visit: NewSiteVisit) {
  await db.insert(siteVisits).values(visit);
  if (Math.random() < 0.01) {
    await db.delete(siteVisits).where(lt(siteVisits.at, new Date(Date.now() - KEEP_DAYS * DAY_MS)));
  }
}

/** The admin's Analytics page: people arriving at the website, and agents reading it, over the last `days`. */
export async function siteAnalytics(db: Db, days: number) {
  const v = siteVisits;
  const since = gte(v.at, new Date(Date.now() - days * DAY_MS));
  const people = and(since, isNull(v.agent));
  const agents = and(since, isNotNull(v.agent));
  const visits = count();
  const day = sql<string>`date(${v.at} / 1000, 'unixepoch')`;

  const [byDay, channels, sources, countries, pages, queries, campaigns, devices, agentNames, agentPages, recent, recentAgents] =
    await Promise.all([
      db
        .select({
          day,
          people: sql<number>`sum(${v.agent} is null)`.mapWith(Number),
          agents: sql<number>`sum(${v.agent} is not null)`.mapWith(Number),
        })
        .from(v)
        .where(since)
        .groupBy(day)
        .orderBy(day),
      db.select({ channel: v.channel, visits }).from(v).where(people).groupBy(v.channel).orderBy(desc(visits)),
      db
        .select({ source: v.source, channel: v.channel, visits })
        .from(v)
        .where(people)
        .groupBy(v.source, v.channel)
        .orderBy(desc(visits))
        .limit(30),
      db.select({ country: v.country, visits }).from(v).where(people).groupBy(v.country).orderBy(desc(visits)).limit(60),
      db.select({ path: v.path, visits }).from(v).where(people).groupBy(v.path).orderBy(desc(visits)).limit(30),
      db
        .select({ query: v.query, source: v.source, visits })
        .from(v)
        .where(and(people, isNotNull(v.query)))
        .groupBy(v.query, v.source)
        .orderBy(desc(visits))
        .limit(50),
      db
        .select({ source: v.utmSource, medium: v.utmMedium, campaign: v.utmCampaign, visits })
        .from(v)
        .where(and(people, or(isNotNull(v.utmSource), isNotNull(v.utmCampaign))))
        .groupBy(v.utmSource, v.utmMedium, v.utmCampaign)
        .orderBy(desc(visits))
        .limit(30),
      db.select({ device: v.device, visits }).from(v).where(people).groupBy(v.device).orderBy(desc(visits)),
      db
        .select({ agent: v.agent, kind: v.agentKind, reads: visits })
        .from(v)
        .where(agents)
        .groupBy(v.agent, v.agentKind)
        .orderBy(desc(visits)),
      // What AI assistants and AI search read, a hint at what people ask them about.
      db
        .select({ path: v.path, reads: visits })
        .from(v)
        .where(and(agents, inArray(v.agentKind, ["assistant", "ai-search"])))
        .groupBy(v.path)
        .orderBy(desc(visits))
        .limit(20),
      db
        .select({ at: v.at, country: v.country, source: v.source, channel: v.channel, path: v.path, query: v.query, device: v.device })
        .from(v)
        .where(people)
        .orderBy(desc(v.at))
        .limit(50),
      db
        .select({ at: v.at, agent: v.agent, kind: v.agentKind, path: v.path, country: v.country })
        .from(v)
        .where(agents)
        .orderBy(desc(v.at))
        .limit(30),
    ]);

  const channel = (name: string) => channels.find((c) => c.channel === name)?.visits ?? 0;
  return {
    days,
    totals: {
      visits: channels.reduce((n, c) => n + c.visits, 0),
      countries: countries.filter((c) => c.country).length,
      fromSearch: channel("search"),
      fromAi: channel("ai"),
      agentReads: agentNames.reduce((n, a) => n + a.reads, 0),
    },
    byDay,
    channels,
    sources,
    countries,
    pages,
    queries,
    campaigns,
    devices,
    agents: agentNames,
    agentPages,
    recent,
    recentAgents,
  };
}
