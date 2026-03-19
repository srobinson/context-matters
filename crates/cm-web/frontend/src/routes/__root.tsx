import { Link, Outlet, createRootRoute } from "@tanstack/react-router";
import { ThemeToggle } from "@/components/ThemeToggle";

export const rootRoute = createRootRoute({
  component: RootLayout,
});

function RootLayout() {
  return (
    <div className="min-h-screen bg-background text-foreground">
      <header className="sticky top-0 z-50 border-b border-border bg-background/95 backdrop-blur supports-backdrop-filter:bg-background/60">
        <div className="flex h-12 items-center justify-between px-6">
          <div className="flex items-center gap-6">
            <span className="font-mono text-sm font-semibold tracking-tighter text-foreground">
              cm
              <span className="text-muted-foreground/60">:</span>
              <span className="text-muted-foreground">web</span>
            </span>
            <nav className="flex items-center gap-1">
              <NavLink to="/">dashboard</NavLink>
              <NavLink to="/feed">feed</NavLink>
            </nav>
          </div>
          <ThemeToggle />
        </div>
      </header>
      <main className="px-6 py-6">
        <Outlet />
      </main>
    </div>
  );
}

function NavLink({ to, children }: { to: string; children: React.ReactNode }) {
  return (
    <Link
      to={to}
      className="rounded-md px-3 py-1.5 font-mono text-xs text-muted-foreground transition-colors hover:text-foreground [&.active]:bg-accent [&.active]:text-foreground"
    >
      {children}
    </Link>
  );
}
