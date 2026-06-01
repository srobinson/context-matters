# Lessons

- When the user asks for deep research on public tooling behavior, verify current sources before treating a local workaround as the fix.
- For Cargo target selection, verify the current Cargo behavior and local command output before changing `justfile` recipes.
- When preserving structured JSON ordering, do not expose tuple arrays to users; use named object buckets so the wire shape remains readable.
