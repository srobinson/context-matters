import { createRouter } from "@tanstack/react-router";
import { rootRoute } from "@/routes/__root";
import { indexRoute } from "@/routes/index";
import { feedRoute } from "@/routes/feed";

const routeTree = rootRoute.addChildren([indexRoute, feedRoute]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
