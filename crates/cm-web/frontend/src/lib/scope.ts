export type ScopeSelector =
  | { kind: "path"; path: string }
  | { kind: "cwd_inferred"; cwd?: string }
  | { kind: "subtree"; path: string }
  | { kind: "set"; paths: string[] }
  | { kind: "all" };

export function serializeScopeSelector(scope: ScopeSelector): string;
export function serializeScopeSelector(scope: ScopeSelector | undefined): string | undefined;
export function serializeScopeSelector(scope: ScopeSelector | undefined): string | undefined {
  return scope == null ? undefined : JSON.stringify(scope);
}
