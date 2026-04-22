import { createRoute } from "@tanstack/react-router";
import { rootRoute } from "./__root";
import { FeedPage } from "./feed/FeedPage";
import { validateFeedSearch } from "./feed/search";

export type { FeedSearch } from "./feed/search";

export const feedRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/feed",
  validateSearch: validateFeedSearch,
  component: FeedRouteComponent,
});

function FeedRouteComponent() {
  const search = feedRoute.useSearch();
  return <FeedPage search={search} />;
}
