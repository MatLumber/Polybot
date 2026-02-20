export interface ApiResponseWire<T> {
  success: boolean
  data?: T
  error?: string
}

export interface PaperStatsWire {
  total_trades: number
  wins: number
  losses: number
  win_rate: number
  total_pnl: number
  total_fees: number
  largest_win: number
  largest_loss: number
  avg_win: number
  avg_loss: number
  max_drawdown: number
  current_drawdown: number
  peak_balance: number
  profit_factor: number
  current_streak: number
  best_streak: number
  worst_streak: number
  exits_trailing_stop: number
  exits_take_profit: number
  exits_market_expiry: number
  exits_time_expiry: number
}

export interface PositionWire {
  id: string
  asset: string
  timeframe: string
  direction: string
  entry_price: number
  current_price: number
  size_usdc: number
  pnl: number
  pnl_pct: number
  opened_at: number
  market_slug: string
  confidence: number
  peak_price: number
  trough_price: number
  market_close_ts: number
  time_remaining_secs: number
}

export interface TradeWire {
  timestamp: number
  trade_id: string
  asset: string
  timeframe: string
  direction: string
  confidence: number
  entry_price: number
  exit_price: number
  size_usdc: number
  pnl: number
  pnl_pct: number
  result: string
  exit_reason: string
  hold_duration_secs: number
  balance_after: number
  rsi_at_entry?: number
  macd_hist_at_entry?: number
  bb_position_at_entry?: number
  adx_at_entry?: number
  volatility_at_entry?: number
}

export interface AssetStatsWire {
  asset: string
  trades: number
  wins: number
  losses: number
  win_rate: number
  pnl: number
  avg_confidence: number
}

export interface AssetPriceWire {
  asset: string
  price: number
  bid: number
  ask: number
  source: string
  timestamp: number
  change_24h: number
}

export interface PriceHistoryPointWire {
  timestamp: number
  price: number
  source: string
}

export interface MarketLearningProgressWire {
  market_key: string
  asset: string
  timeframe: string
  sample_count: number
  target_samples: number
  progress_pct: number
  indicators_active: number
  avg_win_rate_pct: number
  last_updated_ts: number
  status: string
}

export interface PaperDashboardWire {
  balance: number
  available: number
  locked: number
  total_equity: number
  unrealized_pnl: number
  stats: PaperStatsWire
  open_positions: PositionWire[]
  recent_trades: TradeWire[]
  asset_stats: Record<string, AssetStatsWire>
}

export interface LiveDashboardWire {
  balance: number
  available: number
  locked: number
  total_equity: number
  unrealized_pnl: number
  open_positions: PositionWire[]
  daily_pnl: number
  daily_trades: number
  kill_switch_active: boolean
}

export interface PriceDashboardWire {
  prices: Record<string, AssetPriceWire>
  last_update: number
}

export interface ExecutionDiagnosticsWire {
  processed_features: number
  generated_signals: number
  filtered_features: number
  strategy_filter_reasons: Record<string, number>
  last_strategy_filter_reason?: string | null
  last_strategy_filter_ts: number
  accepted_signals: number
  rejected_signals: number
  rejection_reasons: Record<string, number>
  last_rejection_reason?: string | null
  last_rejection_ts: number
}

export interface DashboardStateWire {
  paper: PaperDashboardWire
  live: LiveDashboardWire
  prices: PriceDashboardWire
  execution: ExecutionDiagnosticsWire
  timestamp: number
}

export interface MLModelInfoWire {
  name: string
  weight: number
  accuracy: number
  status: string
}

export interface MLStateWire {
  enabled: boolean
  model_type: string
  version: string
  timestamp: number
}

export interface MLMetricsWire {
  accuracy: number
  win_rate: number
  loss_rate: number
  total_predictions: number
  correct_predictions: number
  incorrect_predictions: number
  ensemble_weights: MLModelInfoWire[]
  epoch?: number
  dataset_size?: number
  timestamp: number
}

export interface MLPredictionWire {
  asset: string
  timeframe: string
  direction: string
  confidence: number
  prob_up: number
  model_name: string
  features_triggered: string[]
  timestamp: number
}

export interface MLFeaturesWire {
  total_features: number
  top_features: Array<{ name: string; importance: number }>
  timestamp: number
}

export interface MLTrainingWire {
  status: string
  last_training: number | null
  samples_trained: number
  retrain_interval: number
  walk_forward_enabled: boolean
  timestamp: number
}

export type WsMessageWire =
  | { type: 'FullState'; data: DashboardStateWire }
  | { type: 'StatsUpdate'; data: PaperStatsWire }
  | { type: 'NewTrade'; data: TradeWire }
  | { type: 'NewSignal'; data: unknown }
  | { type: 'PriceUpdate'; data: Record<string, AssetPriceWire> }
  | { type: 'PositionOpened'; data: PositionWire }
  | { type: 'PositionClosed'; data: { position_id: string; trade: TradeWire } }
  | { type: 'PositionsUpdate'; data: PositionWire[] }
  | { type: 'MLStateUpdate'; data: MLStateWire }
  | { type: 'MLPrediction'; data: MLPredictionWire }
  | { type: 'MLMetricsUpdate'; data: MLMetricsWire }
  | { type: 'Heartbeat'; data: number }
