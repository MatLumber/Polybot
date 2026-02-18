import { memo, useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "./Card";
import { cn, formatCurrency, formatPercent, getPnLColor } from "../lib/utils";
import type { AssetPrice } from "../types/dashboard";
import { TrendingUp, TrendingDown, Minus, DollarSign, Coins } from "lucide-react";

interface PriceCardProps {
  asset: string;
  price: AssetPrice;
}

// Memoize PriceCard to prevent flickering - only re-render if price changes significantly
const PriceCardComponent = ({ asset, price }: PriceCardProps) => {
  const change24h = price.change_24h ?? 0;
  const changeColor = getPnLColor(change24h);
  const TrendIcon = change24h > 0 ? TrendingUp : change24h < 0 ? TrendingDown : Minus;

  return (
    <Card className="hover:shadow-lg transition-all hover:border-primary/30 hover:scale-[1.02]">
      <CardHeader className="pb-1 pt-2.5">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1">
            <Coins className="h-2.5 w-2.5 text-muted-foreground" />
            <CardTitle className="text-[11px] font-semibold">{asset}</CardTitle>
          </div>
          <TrendIcon className={cn("h-2.5 w-2.5", changeColor)} />
        </div>
      </CardHeader>
      <CardContent className="pb-2.5">
        <div className="text-base font-bold font-mono-nums leading-none">
          {formatCurrency(price.price)}
        </div>
        <div className={cn("text-[10px] mt-1 font-mono-nums", changeColor)}>
          {formatPercent(change24h)} 24h
        </div>
      </CardContent>
    </Card>
  );
};

// Custom comparison: only re-render if price changes by more than 0.1% or change_24h changes by 0.1%
const arePropsEqual = (prevProps: PriceCardProps, nextProps: PriceCardProps) => {
  const prevPrice = prevProps.price.price;
  const nextPrice = nextProps.price.price;
  
  // Prevent division by zero
  if (prevPrice === 0 || nextPrice === 0) {
    return prevPrice === nextPrice;
  }
  
  const priceDiff = Math.abs(nextPrice - prevPrice) / prevPrice;
  const prevChange = prevProps.price.change_24h ?? 0;
  const nextChange = nextProps.price.change_24h ?? 0;
  const changeDiff = Math.abs(nextChange - prevChange);
  
  // Only re-render if price changed by > 0.1% or change_24h changed by > 0.1%
  return priceDiff < 0.001 && changeDiff < 0.001;
};

export const PriceCard = memo(PriceCardComponent, arePropsEqual);

interface PriceGridProps {
  prices: Record<string, AssetPrice>;
}

export function PriceGrid({ prices }: PriceGridProps) {
  // Memoize the price list to prevent unnecessary re-renders
  const priceList = useMemo(() => {
    if (!prices || Object.keys(prices).length === 0) return [];
    return Object.entries(prices).sort(([a], [b]) => a.localeCompare(b));
  }, [prices]);
  
  // Memoize the sorted asset keys for stable rendering
  const assetKeys = useMemo(() => priceList.map(([asset]) => asset), [priceList]);
  
  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="flex items-center gap-2 text-sm">
          <DollarSign className="h-3.5 w-3.5" />
          Live Prices
        </CardTitle>
      </CardHeader>
      <CardContent className="pt-0">
        {priceList.length === 0 ? (
          <div className="text-center py-2.5 text-muted-foreground text-xs">
            Waiting for prices...
          </div>
        ) : (
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-2">
            {priceList.map(([asset, price], index) => (
              <PriceCard key={assetKeys[index]} asset={asset} price={price} />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
