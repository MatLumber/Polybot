// Dashboard API Types - matches Rust backend types

export interface PaperStats {
  total_trades: number;
  wins: number;
  losses: number;
  win_rate: number;
  total_pnl: number;
  total_fees: number;
  largest_win: number;
  largest_loss: number;
  avg_win: number;
  avg_loss: number;
  max_drawdown: number;
  current_drawdown: number;
  peak_balance: number;
  profit_factor: number;
  current_streak: number;
  best_streak: number;
  worst_streak: number;
  exits_trailing_stop: number;
  exits_take_profit: number;
  exits_market_expiry: number;
  exits_time_expiry: number;
}

export interface Position {
  id: string;
  asset: string;
  timeframe: string;
  direction: string;
  entry_price: number;
  current_price: number;
  size_usdc: number;
  pnl: number;
  pnl_pct: number;
  opened_at: number;
  market_slug: string;
  confidence: number;
  peak_price: number;
  trough_price: number;
  market_close_ts: number;
  time_remaining_secs: number;
}

export interface Trade {
  id: string;
  asset: string;
  direction: string;
  entry_price: number;
  exit_price: number;
  size_usdc: number;
  pnl: number;
  pnl_pct: number;
  fees: number;
  entry_time: number;
  exit_time: number;
  exit_reason: string;
  market_slug: string;
}

export interface Signal {
  id: string;
  asset: string;
  timeframe: string;
  direction: string;
  confidence: number;
  price: number;
  timestamp: number;
  indicators: Record<string, number>;
}

export interface AssetPrice {
  asset: string;
  price: number;
  bid?: number;
  ask?: number;
  change_24h?: number;
  timestamp: number;
  source: string;
}

export interface AssetStats {
  asset: string;
  total_trades: number;
  wins: number;
  losses: number;
  win_rate: number;
  total_pnl: number;
  avg_pnl: number;
}

export interface PaperDashboard {
  balance: number;
  available: number;
  locked: number;
  total_equity: number;
  unrealized_pnl: number;
  stats: PaperStats;
  open_positions: Position[];
  recent_trades: Trade[];
  asset_stats: Record<string, AssetStats>;
}

export interface LiveDashboard {
  balance: number;
  available: number;
  locked: number;
  total_equity: number;
  unrealized_pnl: number;
  open_positions: Position[];
  daily_pnl: number;
  daily_trades: number;
  kill_switch_active: boolean;
}

export interface PriceDashboard {
  prices: Record<string, AssetPrice>;
  last_update: number;
}

export interface DashboardState {
  paper: PaperDashboard;
  live: LiveDashboard;
  prices: PriceDashboard;
  timestamp: number;
}

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

// WebSocket Message Types
export type WsMessage =
  | { type: "FullState"; data: DashboardState }
  | { type: "StatsUpdate"; data: PaperStats }
  | { type: "NewTrade"; data: Trade }
  | { type: "NewSignal"; data: Signal }
  | { type: "PriceUpdate"; data: Record<string, AssetPrice> }
  | { type: "PositionOpened"; data: Position }
  | { type: "PositionClosed"; data: { position_id: string; trade: Trade } }
  | { type: "Heartbeat"; data: { timestamp: number } };
