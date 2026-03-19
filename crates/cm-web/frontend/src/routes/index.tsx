import { createRoute } from "@tanstack/react-router";
import { rootRoute } from "./__root";

export const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: DashboardPage,
});

function DashboardPage() {
  return (
    <div className="space-y-6">
      <div className="flex items-baseline gap-3">
        <h2 className="text-lg font-medium tracking-tight">Dashboard</h2>
        <span className="font-mono text-xs text-muted-foreground">
          system overview
        </span>
      </div>
      <div className="grid grid-cols-4 gap-4">
        {["Active entries", "Today", "This week", "Agents"].map((label) => (
          <div
            key={label}
            className="rounded-lg border border-border bg-card p-4"
          >
            <p className="text-xs font-medium text-muted-foreground">
              {label}
            </p>
            <p className="mt-1 font-mono text-2xl font-semibold tracking-tight text-card-foreground">
              --
            </p>
          </div>
        ))}
      </div>
      <p className="text-xs text-muted-foreground">
        Waiting for API connection...
      </p>
    </div>
  );
}
