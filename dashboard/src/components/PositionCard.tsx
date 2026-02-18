import { Card, CardContent, CardHeader, CardTitle } from "./Card";
import { cn, formatCurrency, formatPercent, getPnLColor, formatTimeRemaining } from "../lib/utils";
import type { Position } from "../types/dashboard";
import { ArrowUp, ArrowDown, Clock, TrendingUp, DollarSign, Target } from "lucide-react";

interface PositionCardProps {
  position: Position;
}

export function PositionCard({ position }: PositionCardProps) {
  const isUp = position.direction === "Up";
  const DirectionIcon = isUp ? ArrowUp : ArrowDown;
  const pnlColor = getPnLColor(position.pnl);
  const timeRemaining = formatTimeRemaining(position.time_remaining_secs);

  return (
    <Card className={cn(
      "relative overflow-hidden transition-all duration-200 hover:border-primary/50 hover:scale-[1.01]",
      position.pnl >= 0 ? "hover:shadow-profit/20" : "hover:shadow-loss/20"
    )}>
      {/* Direction indicator bar */}
      <div className={cn(
        "absolute top-0 left-0 right-0 h-0.5",
        isUp ? "bg-profit" : "bg-loss"
      )} />
      
      <CardHeader className="pb-1 pt-2.5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1">
            <DirectionIcon className={cn("h-3 w-3", isUp ? "text-profit" : "text-loss")} />
            <CardTitle className="text-xs font-semibold">{position.asset}</CardTitle>
            <span className="text-[9px] text-muted-foreground ml-1">{position.timeframe}</span>
          </div>
          <div className={cn("font-mono-nums text-xs font-bold", pnlColor)}>
            {formatPercent(position.pnl_pct)}
          </div>
        </div>
      </CardHeader>
      
      <CardContent className="space-y-1.5 pt-1.5 pb-2.5">
        {/* Investment & PnL Row */}
        <div className="flex justify-between items-center py-1.5 px-2 rounded-md bg-secondary/30">
          <div className="flex items-center gap-1.5">
            <DollarSign className="h-2.5 w-2.5 text-muted-foreground" />
            <div>
              <div className="text-[9px] text-muted-foreground uppercase">Invested</div>
              <div className="font-mono-nums text-xs font-semibold">{formatCurrency(position.size_usdc)}</div>
            </div>
          </div>
          <div className="flex items-center gap-1.5">
            <Target className={cn("h-2.5 w-2.5", pnlColor)} />
            <div className="text-right">
              <div className="text-[9px] text-muted-foreground uppercase">P&L</div>
              <div className={cn("font-mono-nums text-xs font-bold", pnlColor)}>
                {formatCurrency(position.pnl)}
              </div>
            </div>
          </div>
        </div>
        
        {/* Price info */}
        <div className="flex justify-between items-center text-[10px]">
          <div className="flex items-center gap-3">
            <div>
              <span className="text-muted-foreground">Entry </span>
              <span className="font-mono-nums">{formatCurrency(position.entry_price)}</span>
            </div>
            <div>
              <span className="text-muted-foreground">Now </span>
              <span className="font-mono-nums">{formatCurrency(position.current_price)}</span>
            </div>
          </div>
        </div>
        
        {/* Time & Confidence */}
        <div className="flex justify-between items-center pt-1 border-t border-border/50 text-[10px]">
          <div className="flex items-center gap-1.5">
            <Clock className="h-2.5 w-2.5 text-muted-foreground" />
            <span className="font-mono-nums text-muted-foreground">{timeRemaining}</span>
          </div>
          <div className="flex items-center gap-1">
            <span className="text-muted-foreground">Confidence</span>
            <span className="font-mono-nums font-medium">{(position.confidence * 100).toFixed(0)}%</span>
          </div>
        </div>
      </CardContent>
    </Card>
  );
}

interface PositionListProps {
  positions: Position[];
  title?: string;
}

export function PositionList({ positions, title = "Open Positions" }: PositionListProps) {
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="flex items-center gap-2 text-sm">
          <TrendingUp className="h-3.5 w-3.5" />
          {title}
          <span className="text-[10px] text-muted-foreground font-normal">
            ({positions.length})
          </span>
        </CardTitle>
      </CardHeader>
      <CardContent className="pt-0">
        {positions.length === 0 ? (
          <div className="text-center py-4 text-muted-foreground text-xs">
            No open positions
          </div>
        ) : (
          <div className="grid gap-2 md:grid-cols-2 lg:grid-cols-1">
            {positions.map((position) => (
              <PositionCard key={position.id} position={position} />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}


