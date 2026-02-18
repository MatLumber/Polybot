import { Card, CardContent, CardHeader, CardTitle } from "./Card";
import { cn, formatPercent, getWinRateColor, getPnLColor } from "../lib/utils";

interface StatsCardProps {
  title: string;
  value: string | number;
  subtitle?: string;
  trend?: number;
  icon?: React.ReactNode;
  className?: string;
  valueClassName?: string;
}

export function StatsCard({
  title,
  value,
  subtitle,
  trend,
  icon,
  className,
  valueClassName,
}: StatsCardProps) {
  return (
    <Card className={cn("relative overflow-hidden", className)}>
      <CardHeader className="flex flex-row items-center justify-between pb-1 pt-2.5">
        <CardTitle className="text-[11px] font-medium text-muted-foreground uppercase tracking-wide">
          {title}
        </CardTitle>
        {icon && <div className="h-3 w-3 text-muted-foreground opacity-60">{icon}</div>}
      </CardHeader>
      <CardContent className="pb-2.5">
        <div className={cn("text-lg font-bold font-mono-nums leading-none", valueClassName)}>
          {value}
        </div>
        {(subtitle || trend !== undefined) && (
          <p className="text-[10px] text-muted-foreground mt-1 leading-tight">
            {subtitle}
            {trend !== undefined && (
              <span className={cn("ml-2", getPnLColor(trend))}>
                {formatPercent(trend)}
              </span>
            )}
          </p>
        )}
      </CardContent>
    </Card>
  );
}

interface WinRateCardProps {
  wins: number;
  losses: number;
  winRate: number;
}

export function WinRateCard({ wins, losses, winRate }: WinRateCardProps) {
  return (
    <Card>
      <CardHeader className="pb-1 pt-2.5">
        <CardTitle className="text-[11px] font-medium text-muted-foreground uppercase tracking-wide">
          Win Rate
        </CardTitle>
      </CardHeader>
      <CardContent className="pb-2.5">
        <div className={cn("text-lg font-bold font-mono-nums leading-none", getWinRateColor(winRate))}>
          {winRate.toFixed(1)}%
        </div>
        <div className="mt-1 flex items-center gap-2.5 text-[10px]">
          <span className="text-profit">{wins}W</span>
          <span className="text-loss">{losses}L</span>
        </div>
        <div className="mt-1.5 h-1 bg-secondary rounded-full overflow-hidden">
          <div
            className="h-full bg-profit transition-all duration-500"
            style={{ width: `${winRate}%` }}
          />
        </div>
      </CardContent>
    </Card>
  );
}

interface DrawdownCardProps {
  currentDrawdown: number;
  maxDrawdown: number;
}

export function DrawdownCard({ currentDrawdown, maxDrawdown }: DrawdownCardProps) {
  return (
    <Card>
      <CardHeader className="pb-1 pt-2.5">
        <CardTitle className="text-[11px] font-medium text-muted-foreground uppercase tracking-wide">
          Drawdown
        </CardTitle>
      </CardHeader>
      <CardContent className="pb-2.5">
        <div className="text-lg font-bold font-mono-nums text-loss leading-none">
          {formatPercent(currentDrawdown)}
        </div>
        <p className="text-[10px] text-muted-foreground mt-1">
          Max: {formatPercent(maxDrawdown)}
        </p>
        <div className="mt-1.5 h-1 bg-secondary rounded-full overflow-hidden">
          <div
            className="h-full bg-loss transition-all duration-500"
            style={{ width: `${Math.min(currentDrawdown, 100)}%` }}
          />
        </div>
      </CardContent>
    </Card>
  );
}

interface StreakCardProps {
  current: number;
  best: number;
  worst: number;
}

export function StreakCard({ current, best, worst }: StreakCardProps) {
  const isWinStreak = current > 0;
  return (
    <Card>
      <CardHeader className="pb-1 pt-2.5">
        <CardTitle className="text-[11px] font-medium text-muted-foreground uppercase tracking-wide">
          Streak
        </CardTitle>
      </CardHeader>
      <CardContent className="pb-2.5">
        <div className={cn(
          "text-lg font-bold font-mono-nums leading-none",
          current === 0 ? "text-neutral" : isWinStreak ? "text-profit" : "text-loss"
        )}>
          {current > 0 ? `+${current}` : current}
        </div>
        <div className="mt-1 flex items-center gap-2.5 text-[10px]">
          <span className="text-profit">Best: +{best}</span>
          <span className="text-loss">Worst: {worst}</span>
        </div>
      </CardContent>
    </Card>
  );
}
