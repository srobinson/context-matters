import { api } from "./client";

const scope = "global/project:helioy";
const entry = {
  scope,
  kind: "fact" as const,
  title: "Scope contract",
  body: "Frontend request surfaces send scope.",
  created_by: "agent:test",
};

void api.entries.browse({ scope });
void api.entries.search({ query: "Scope", scope });
void api.agent.browse({ scope: "cwd_inferred", cwd: "/tmp/helioy/context-matters" });
void api.export(scope);
void api.entries.create(entry);
void api.entries.merge("019dd3ad-8ea2-7751-ad87-1bd49e8bc242", entry);

// @ts-expect-error public browse requests no longer accept scope_path
void api.entries.browse({ scope_path: scope });

// @ts-expect-error public browse requests no longer accept scope_mode
void api.agent.browse({ scope_mode: "resolved" });

// @ts-expect-error public search requests no longer accept scope_path
void api.entries.search({ query: "Scope", scope_path: scope });

// @ts-expect-error public export requests no longer accept an options object with scope_path
void api.export({ scope_path: scope });

// @ts-expect-error public create bodies no longer accept scope_path
void api.entries.create({ ...entry, scope: undefined, scope_path: scope });
