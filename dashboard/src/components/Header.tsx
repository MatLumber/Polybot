import { cn } from "../lib/utils";
import { Wifi, WifiOff, Bot } from "lucide-react";

interface HeaderProps {
  connected: boolean;
  mode: "paper" | "live";
  onModeChange?: (mode: "paper" | "live") => void;
}

export function Header({ connected, mode, onModeChange }: HeaderProps) {
  return (
    <header className="border-b border-border/40 bg-card/20 backdrop-blur-lg sticky top-0 z-50 shadow-sm">
      <div className="container mx-auto px-3 h-12 flex items-center justify-between">
        {/* Logo */}
        <div className="flex items-center gap-2">
          <div className="p-1 bg-primary/10 rounded border border-primary/20">
            <Bot className="h-4 w-4 text-primary" />
          </div>
          <div>
            <h1 className="text-base font-bold tracking-tight">PolyBot</h1>
            <p className="text-[9px] text-muted-foreground leading-none -mt-0.5">Trading Dashboard</p>
          </div>
        </div>

        {/* Mode Toggle */}
        <div className="flex items-center gap-2.5">
          <div className="flex bg-secondary/80 backdrop-blur-sm rounded-md p-0.5 border border-border/30">
            <button
              onClick={() => onModeChange?.("paper")}
              className={cn(
                "px-2.5 py-0.5 text-[11px] font-medium rounded transition-all duration-200",
                mode === "paper"
                  ? "bg-background text-foreground shadow-sm border border-border/50"
                  : "text-muted-foreground hover:text-foreground hover:bg-background/50"
              )}
            >
              Paper
            </button>
            <button
              onClick={() => onModeChange?.("live")}
              className={cn(
                "px-2.5 py-0.5 text-[11px] font-medium rounded transition-all duration-200",
                mode === "live"
                  ? "bg-background text-foreground shadow-sm border border-border/50"
                  : "text-muted-foreground hover:text-foreground hover:bg-background/50"
              )}
            >
              Live
            </button>
          </div>

          {/* Connection Status */}
          <div className="flex items-center gap-1 px-2 py-1 bg-secondary/50 backdrop-blur-sm rounded border border-border/30">
            {connected ? (
              <>
                <Wifi className="h-3 w-3 text-profit animate-pulse" />
                <span className="text-[10px] text-profit font-medium">Connected</span>
              </>
            ) : (
              <>
                <WifiOff className="h-3 w-3 text-loss" />
                <span className="text-[10px] text-loss font-medium">Disconnected</span>
              </>
            )}
          </div>
        </div>
      </div>
    </header>
  );
}
