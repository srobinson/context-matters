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

  for (const node of nodeMap.values()) {
    node.children.sort((a, b) => b.entryCount - a.entryCount);
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
    <section className="rounded-surface border border-border/70 bg-card/70 p-4 shadow-surface backdrop-blur-sm">
      <div className="mb-4 space-y-2">
        <div className="flex items-center gap-1.5">
          <FolderTree className="h-3.5 w-3.5 text-muted-foreground/60" />
          <h3 className="font-mono text-[10px] uppercase tracking-[0.24em] text-muted-foreground/60">
            hot scopes
          </h3>
        </div>
        <p className="text-sm text-muted-foreground">
          Entry concentration by scope. Start here when you need to understand where the weight is accumulating.
        </p>
      </div>
      <div className="space-y-1">
        {tree.map((node) => (
          <ScopeNodeRow key={node.path} node={node} depth={0} />
        ))}
      </div>
    </section>
  );
}

function ScopeNodeRow({ node, depth }: { node: ScopeNode; depth: number }) {
  const [expanded, setExpanded] = useState(depth < 2);
  const hasChildren = node.children.length > 0;

  return (
    <div>
      <div
        className="flex items-center gap-2 rounded-control border border-transparent px-2 py-2 transition-colors hover:border-border/80 hover:bg-accent/20"
        style={{ paddingLeft: `${depth * 18 + 8}px` }}
      >
        {hasChildren ? (
          <button
            type="button"
            onClick={() => setExpanded((p) => !p)}
            className="shrink-0 rounded-control p-0.5 text-muted-foreground/60 hover:text-foreground"
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
          className="min-w-0 flex-1 truncate font-mono text-xs text-foreground hover:underline underline-offset-2"
        >
          {node.segment}
        </Link>
        <span className="shrink-0 rounded-control border border-border/70 bg-background/70 px-1.5 py-0.5 font-mono text-[10px] text-muted-foreground/80">
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
