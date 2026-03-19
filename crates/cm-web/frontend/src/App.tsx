import { TooltipProvider } from "@/components/ui/tooltip";

function App() {
  return (
    <TooltipProvider>
      <div className="min-h-screen bg-background text-foreground">
        <header className="border-b border-border px-6 py-4">
          <h1 className="font-mono text-sm font-medium tracking-tight">
            cm-web
          </h1>
        </header>
        <main className="px-6 py-8">
          <p className="text-sm text-muted-foreground">
            Context store monitor
          </p>
        </main>
      </div>
    </TooltipProvider>
  );
}

export default App;
