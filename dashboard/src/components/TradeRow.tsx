import { cn, formatCurrency, formatPercent, getPnLColor } from "../lib/utils";
import type { Trade } from "../types/dashboard";
import { ArrowUp, ArrowDown, Clock } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "./Card";

interface TradeRowProps {
  trade: Trade;
}

export function TradeRow({ trade }: TradeRowProps) {
  const isUp = trade.direction === "Up";
  const isWin = trade.pnl > 0;
  const pnlColor = getPnLColor(trade.pnl);
  const DirectionIcon = isUp ? ArrowUp : ArrowDown;

  return (
    <div className="flex items-center justify-between py-1.5 px-2.5 hover:bg-accent/30 transition-all border-b border-border/30 last:border-0 hover:border-primary/20">
      <div className="flex items-center gap-1.5">
        <div className={cn(
          "p-0.5 rounded",
          isWin ? "bg-profit/10" : "bg-loss/10"
        )}>
          <DirectionIcon className={cn("h-2.5 w-2.5", isUp ? "text-profit" : "text-loss")} />
        </div>
        <div>
          <div className="flex items-center gap-1">
            <span className="font-medium text-xs">{trade.asset}</span>
            <span className="text-[9px] text-muted-foreground">{trade.market_slug}</span>
          </div>
          <div className="text-[9px] text-muted-foreground leading-tight">
            {new Date(trade.entry_time).toLocaleTimeString()} → {new Date(trade.exit_time).toLocaleTimeString()}
          </div>
        </div>
      </div>
      
      <div className="flex items-center gap-3">
        <div className="text-right">
          <div className="text-[9px] text-muted-foreground">Entry → Exit</div>
          <div className="font-mono-nums text-[10px]">
            {formatCurrency(trade.entry_price)} → {formatCurrency(trade.exit_price)}
          </div>
        </div>
        
        <div className="text-right min-w-16">
          <div className={cn("font-mono-nums font-bold text-xs", pnlColor)}>
            {formatCurrency(trade.pnl)}
          </div>
          <div className={cn("text-[9px]", pnlColor)}>
            {formatPercent(trade.pnl_pct)}
          </div>
        </div>
        
        <div className="text-[9px] text-muted-foreground w-14 text-right">
          {trade.exit_reason.replace(/_/g, " ")}
        </div>
      </div>
    </div>
  );
}

interface TradeListProps {
  trades: Trade[];
  title?: string;
  maxItems?: number;
}

export function TradeList({ trades, title = "Recent Trades", maxItems = 20 }: TradeListProps) {
  const displayTrades = trades.slice(0, maxItems);
  
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="flex items-center justify-between text-sm">
          <div className="flex items-center gap-2">
            <Clock className="h-3.5 w-3.5" />
            {title}
          </div>
          <span className="text-[10px] text-muted-foreground font-normal">
            {trades.length} trades
          </span>
        </CardTitle>
      </CardHeader>
      <CardContent className="pt-0">
        {trades.length === 0 ? (
          <div className="text-center py-4 text-muted-foreground text-xs">
            No trades yet
          </div>
        ) : (
          <div className="max-h-80 overflow-y-auto">
            {displayTrades.map((trade) => (
              <TradeRow key={trade.id} trade={trade} />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
