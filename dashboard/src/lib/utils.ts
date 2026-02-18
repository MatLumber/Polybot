import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export function formatCurrency(value: number): string {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency: "USD",
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  }).format(value);
}

export function formatPercent(value: number | undefined | null): string {
  if (value === undefined || value === null || isNaN(value)) {
    return "0.00%";
  }
  const sign = value >= 0 ? "+" : "";
  return `${sign}${value.toFixed(2)}%`;
}

export function formatNumber(value: number, decimals: number = 2): string {
  if (value === undefined || value === null || isNaN(value)) {
    return "0." + "0".repeat(decimals);
  }
  return value.toFixed(decimals);
}

export function formatTime(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

export function getWinRateColor(winRate: number): string {
  if (winRate >= 60) return "text-profit";
  if (winRate >= 50) return "text-warning";
  return "text-loss";
}

export function getPnLColor(pnl: number): string {
  if (pnl > 0) return "text-profit";
  if (pnl < 0) return "text-loss";
  return "text-neutral";
}

export function formatTimeRemaining(seconds: number): string {
  if (seconds <= 0) return "00:00:00";
  
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);
  
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
}
