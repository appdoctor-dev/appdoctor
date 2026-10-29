import { defineConfig } from "drizzle-kit";

// Migrations are generated here and applied by Wrangler from apps/api
// (`npm run db:migrate:local -w @repo/api`), which owns the D1 binding.
export default defineConfig({
  dialect: "sqlite",
  schema: ["./src/auth-schema.ts", "./src/schema.ts"],
  out: "./migrations",
});
