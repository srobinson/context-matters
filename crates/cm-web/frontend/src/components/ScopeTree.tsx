import { useMemo, useState } from "react";
import { Link } from "@tanstack/react-router";
import type { Stats } from "@/api/client";
import { ChevronRight, FolderTree } from "lucide-react";
import { cn } from "@/lib/utils";

interface ScopeNode {
  segment: string;
  path: string;
  entryCount: number;
  children: ScopeNode[];
}

function buildTree(
  scopes: { path: string; kind: string; entry_count: number }[],
): ScopeNode[] {
  const nodeMap = new Map<string, ScopeNode>();

  // Sort by path length so parents are processed first
  const sorted = [...scopes].sort(
    (a, b) => a.path.length - b.path.length,
  );

  for (const scope of sorted) {
    const segments = scope.path.split("/");
    const segment = segments[segments.length - 1] ?? scope.path;

    const node: ScopeNode = {
      segment,
      path: scope.path,
      entryCount: scope.entry_count,
      children: [],
    };
    nodeMap.set(scope.path, node);

    // Find parent path
    if (segments.length > 1) {
      const parentPath = segments.slice(0, -1).join("/");
      const parent = nodeMap.get(parentPath);
      if (parent) {
        parent.children.push(node);
      }
    }
  }

  // Root nodes: those without a parent in the map
  const roots: ScopeNode[] = [];
  for (const scope of sorted) {
    const segments = scope.path.split("/");
    if (segments.length === 1) {
      const node = nodeMap.get(scope.path);
      if (node) roots.push(node);
    }
  }

  return roots;
}

export function ScopeTree({ stats }: { stats: Stats }) {
  const tree = useMemo(
    () => buildTree(stats.scope_tree),
    [stats.scope_tree],
  );

  if (tree.length === 0) {
    return (
      <div className="space-y-3">
        <h3 className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          scope tree
        </h3>
        <p className="font-mono text-xs text-muted-foreground">
          No scopes yet.
        </p>
      </div>
    );
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-1.5">
        <FolderTree className="h-3.5 w-3.5 text-muted-foreground/60" />
        <h3 className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground/60">
          scope tree
        </h3>
      </div>
      <div className="space-y-0.5">
        {tree.map((node) => (
          <ScopeNodeRow key={node.path} node={node} depth={0} />
        ))}
      </div>
    </div>
  );
}

function ScopeNodeRow({ node, depth }: { node: ScopeNode; depth: number }) {
  const [expanded, setExpanded] = useState(depth < 2);
  const hasChildren = node.children.length > 0;

  return (
    <div>
      <div
        className="flex items-center gap-1 rounded-md px-1.5 py-1 transition-colors hover:bg-accent/30"
        style={{ paddingLeft: `${depth * 16 + 6}px` }}
      >
        {hasChildren ? (
          <button
            type="button"
            onClick={() => setExpanded((p) => !p)}
            className="shrink-0 rounded-sm p-0.5 text-muted-foreground/60 hover:text-foreground"
          >
            <ChevronRight
              className={cn(
                "h-3 w-3 transition-transform",
                expanded && "rotate-90",
              )}
            />
          </button>
        ) : (
          <span className="w-4" />
        )}
        <Link
          to="/feed"
          search={{ scope_path: node.path }}
          className="flex-1 truncate font-mono text-xs text-foreground hover:underline underline-offset-2"
        >
          {node.segment}
        </Link>
        <span className="shrink-0 font-mono text-[10px] text-muted-foreground/60">
          {node.entryCount}
        </span>
      </div>
      {hasChildren && expanded && (
        <div>
          {node.children.map((child) => (
            <ScopeNodeRow key={child.path} node={child} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
}
