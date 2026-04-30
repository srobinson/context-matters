import { type ScopeSelector, serializeScopeSelector } from "@/lib/scope";
import { api } from "./client";

const pathScope: ScopeSelector = { kind: "path", path: "global/project:helioy" };
const cwdScope: ScopeSelector = {
  kind: "cwd_inferred",
  cwd: "/tmp/helioy/context-matters",
};
const subtreeScope: ScopeSelector = { kind: "subtree", path: "global/project:helioy" };
const setScope: ScopeSelector = {
  kind: "set",
  paths: ["global/project:helioy", "global/project:context-matters"],
};
const allScope: ScopeSelector = { kind: "all" };
const entry = {
  scope: serializeScopeSelector(pathScope),
  kind: "fact" as const,
  title: "Scope contract",
  body: "Frontend request surfaces send scope.",
  created_by: "agent:test",
};

void api.entries.recall({ query: "Scope", scope: pathScope });
void api.agent.recall({ query: "Scope", scope: cwdScope });
void api.entries.search({ query: "Scope", scope: subtreeScope });
void api.agent.search({ query: "Scope", scope: setScope });
void api.agent.browse({ scope: allScope });
void api.entries.browse({ scope: subtreeScope });
void api.export({ scope: cwdScope });
void api.entries.create(entry);
void api.entries.merge("019dd3ad-8ea2-7751-ad87-1bd49e8bc242", entry);

// @ts-expect-error public browse requests no longer accept scope_path
void api.entries.browse({ scope_path: pathScope.path });

// @ts-expect-error public browse requests no longer accept top-level cwd
void api.entries.browse({ cwd: "/tmp/helioy/context-matters" });

// @ts-expect-error public browse requests no longer accept scope_mode
void api.agent.browse({ scope_mode: "resolved" });

// @ts-expect-error public agent browse requests no longer accept top-level cwd
void api.agent.browse({ cwd: "/tmp/helioy/context-matters" });

// @ts-expect-error public search requests no longer accept scope_path
void api.entries.search({ query: "Scope", scope_path: pathScope.path });

// @ts-expect-error public search requests no longer accept top-level cwd
void api.entries.search({ query: "Scope", cwd: "/tmp/helioy/context-matters" });

// @ts-expect-error public agent search requests no longer accept top-level cwd
void api.agent.search({ query: "Scope", cwd: "/tmp/helioy/context-matters" });

// @ts-expect-error public search requests are not paginated until WebRecallView diverges
void api.entries.search({ query: "Scope", cursor: "opaque" });

// @ts-expect-error public agent search requests are not paginated until WebRecallView diverges
void api.agent.search({ query: "Scope", cursor: "opaque" });

// @ts-expect-error public recall requests no longer accept top-level cwd
void api.entries.recall({ query: "Scope", cwd: "/tmp/helioy/context-matters" });

// @ts-expect-error public agent recall requests no longer accept top-level cwd
void api.agent.recall({ query: "Scope", cwd: "/tmp/helioy/context-matters" });

// @ts-expect-error public export requests no longer accept an options object with scope_path
void api.export({ scope_path: pathScope.path });

// @ts-expect-error public export requests no longer accept a positional scope
void api.export(pathScope.path);

// @ts-expect-error public export requests no longer accept top-level cwd
void api.export({ cwd: "/tmp/helioy/context-matters" });

// @ts-expect-error public create bodies no longer accept scope_path
void api.entries.create({ ...entry, scope: undefined, scope_path: pathScope.path });
