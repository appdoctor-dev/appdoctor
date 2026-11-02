CREATE TABLE `site_visits` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`at` integer NOT NULL,
	`path` text NOT NULL,
	`country` text,
	`source` text,
	`channel` text,
	`referrer` text,
	`query` text,
	`utm_source` text,
	`utm_medium` text,
	`utm_campaign` text,
	`device` text,
	`agent` text,
	`agent_kind` text
);
--> statement-breakpoint
CREATE INDEX `site_visits_at_idx` ON `site_visits` (`at`);