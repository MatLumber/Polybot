import type {
  AssetPriceWire,
  AssetStatsWire,
  DashboardStateWire,
  ExecutionDiagnosticsWire,
  MarketLearningProgressWire,
  PaperStatsWire,
  PositionWire,
  PriceHistoryPointWire,
  TradeWire,
} from '../types/wire'
import type {
  AssetPrice,
  AssetStats,
  DashboardState,
  ExecutionDiagnostics,
  MarketLearningProgress,
  MarketLearningStatus,
  PaperStats,
  Position,
  PriceHistoryMap,
  PriceHistoryPoint,
  Trade,
} from '../types/ui'

export const DEFAULT_HISTORY_WINDOW_SECS = 86_400

export function mapPaperStats(stats: PaperStatsWire): PaperStats {
  return {
    totalTrades: stats.total_trades,
    wins: stats.wins,
    losses: stats.losses,
    winRate: stats.win_rate,
    totalPnl: stats.total_pnl,
    totalFees: stats.total_fees,
    largestWin: stats.largest_win,
    largestLoss: stats.largest_loss,
    avgWin: stats.avg_win,
    avgLoss: stats.avg_loss,
    maxDrawdown: stats.max_drawdown,
    currentDrawdown: stats.current_drawdown,
    peakBalance: stats.peak_balance,
    profitFactor: stats.profit_factor,
    currentStreak: stats.current_streak,
    bestStreak: stats.best_streak,
    worstStreak: stats.worst_streak,
    exitsTrailingStop: stats.exits_trailing_stop,
    exitsTakeProfit: stats.exits_take_profit,
    exitsMarketExpiry: stats.exits_market_expiry,
    exitsTimeExpiry: stats.exits_time_expiry,
  }
}

export function mapPosition(position: PositionWire): Position {
  return {
    id: position.id,
    asset: position.asset,
    timeframe: position.timeframe,
    direction: position.direction,
    entryPrice: position.entry_price,
    currentPrice: position.current_price,
    sizeUsdc: position.size_usdc,
    pnl: position.pnl,
    pnlPct: position.pnl_pct,
    openedAt: position.opened_at,
    marketSlug: position.market_slug,
    confidence: position.confidence,
    peakPrice: position.peak_price,
    troughPrice: position.trough_price,
    marketCloseTs: position.market_close_ts,
    timeRemainingSecs: position.time_remaining_secs,
  }
}

export function mapTrade(trade: TradeWire): Trade {
  return {
    id: trade.trade_id,
    timestamp: trade.timestamp,
    asset: trade.asset,
    timeframe: trade.timeframe,
    direction: trade.direction,
    confidence: trade.confidence,
    entryPrice: trade.entry_price,
    exitPrice: trade.exit_price,
    sizeUsdc: trade.size_usdc,
    pnl: trade.pnl,
    pnlPct: trade.pnl_pct,
    result: trade.result,
    exitReason: trade.exit_reason,
    holdDurationSecs: trade.hold_duration_secs,
    balanceAfter: trade.balance_after,
  }
}

export function mapAssetStats(stats: AssetStatsWire): AssetStats {
  return {
    asset: stats.asset,
    totalTrades: stats.trades,
    wins: stats.wins,
    losses: stats.losses,
    winRate: stats.win_rate,
    totalPnl: stats.pnl,
    avgConfidence: stats.avg_confidence,
  }
}

export function mapAssetStatsMap(statsMap: Record<string, AssetStatsWire>): Record<string, AssetStats> {
  return Object.fromEntries(
    Object.entries(statsMap).map(([key, value]) => [key, mapAssetStats(value)]),
  )
}

export function mapAssetPrice(price: AssetPriceWire): AssetPrice {
  return {
    asset: price.asset,
    price: price.price,
    bid: price.bid,
    ask: price.ask,
    source: price.source,
    timestamp: price.timestamp,
    change24h: price.change_24h,
  }
}

export function mapPriceMap(prices: Record<string, AssetPriceWire>): Record<string, AssetPrice> {
  return Object.fromEntries(
    Object.entries(prices).map(([key, value]) => [key, mapAssetPrice(value)]),
  )
}

export function mapExecutionDiagnostics(
  diagnostics: ExecutionDiagnosticsWire,
): ExecutionDiagnostics {
  return {
    processedFeatures: diagnostics.processed_features ?? 0,
    generatedSignals: diagnostics.generated_signals ?? 0,
    filteredFeatures: diagnostics.filtered_features ?? 0,
    strategyFilterReasons: diagnostics.strategy_filter_reasons ?? {},
    lastStrategyFilterReason: diagnostics.last_strategy_filter_reason ?? null,
    lastStrategyFilterTs: diagnostics.last_strategy_filter_ts ?? 0,
    acceptedSignals: diagnostics.accepted_signals,
    rejectedSignals: diagnostics.rejected_signals,
    rejectionReasons: diagnostics.rejection_reasons ?? {},
    lastRejectionReason: diagnostics.last_rejection_reason ?? null,
    lastRejectionTs: diagnostics.last_rejection_ts,
  }
}

export function mapDashboardState(state: DashboardStateWire): DashboardState {
  return {
    paper: {
      balance: state.paper.balance,
      available: state.paper.available,
      locked: state.paper.locked,
      totalEquity: state.paper.total_equity,
      unrealizedPnl: state.paper.unrealized_pnl,
      stats: mapPaperStats(state.paper.stats),
      openPositions: state.paper.open_positions.map(mapPosition),
      recentTrades: state.paper.recent_trades.map(mapTrade),
      assetStats: mapAssetStatsMap(state.paper.asset_stats),
    },
    live: {
      balance: state.live.balance,
      available: state.live.available,
      locked: state.live.locked,
      totalEquity: state.live.total_equity,
      unrealizedPnl: state.live.unrealized_pnl,
      openPositions: state.live.open_positions.map(mapPosition),
      dailyPnl: state.live.daily_pnl,
      dailyTrades: state.live.daily_trades,
      killSwitchActive: state.live.kill_switch_active,
    },
    prices: {
      prices: mapPriceMap(state.prices.prices),
      lastUpdate: state.prices.last_update,
    },
    execution: mapExecutionDiagnostics(state.execution),
    timestamp: state.timestamp,
  }
}

function normalizeLearningStatus(status: string): MarketLearningStatus {
  if (status === 'ready' || status === 'warming_up' || status === 'idle') {
    return status
  }
  return 'idle'
}

export function mapMarketLearningProgress(
  progress: MarketLearningProgressWire,
): MarketLearningProgress {
  return {
    marketKey: progress.market_key,
    asset: progress.asset,
    timeframe: progress.timeframe,
    sampleCount: progress.sample_count,
    targetSamples: progress.target_samples,
    progressPct: progress.progress_pct,
    indicatorsActive: progress.indicators_active,
    avgWinRatePct: progress.avg_win_rate_pct,
    lastUpdatedTs: progress.last_updated_ts,
    status: normalizeLearningStatus(progress.status),
  }
}

export function mapMarketLearningProgressList(
  list: MarketLearningProgressWire[],
): MarketLearningProgress[] {
  return list.map(mapMarketLearningProgress)
}

function normalizeSeries(points: PriceHistoryPoint[]): PriceHistoryPoint[] {
  const sorted = [...points].sort((a, b) => a.timestamp - b.timestamp)
  const deduped: PriceHistoryPoint[] = []

  for (const point of sorted) {
    const last = deduped[deduped.length - 1]
    if (last && last.timestamp === point.timestamp) {
      deduped[deduped.length - 1] = point
    } else {
      deduped.push(point)
    }
  }

  return deduped
}

export function trimSeries(
  points: PriceHistoryPoint[],
  windowSecs: number,
  anchorTimestamp: number,
): PriceHistoryPoint[] {
  const cutoff = anchorTimestamp - windowSecs * 1000
  return points.filter((point) => point.timestamp >= cutoff)
}

export function mapPriceHistory(
  history: Record<string, PriceHistoryPointWire[]>,
  windowSecs = DEFAULT_HISTORY_WINDOW_SECS,
): PriceHistoryMap {
  const mapped: PriceHistoryMap = {}
  const now = Date.now()

  for (const [asset, series] of Object.entries(history)) {
    const points = normalizeSeries(
      series.map((point) => ({
        timestamp: point.timestamp,
        price: point.price,
        source: point.source,
      })),
    )
    mapped[asset] = trimSeries(points, windowSecs, now)
  }

  return mapped
}

export function appendPricePoint(
  history: PriceHistoryMap,
  asset: string,
  point: PriceHistoryPoint,
  windowSecs = DEFAULT_HISTORY_WINDOW_SECS,
): PriceHistoryMap {
  const current = history[asset] ?? []
  const next = normalizeSeries([...current, point])
  const trimmed = trimSeries(next, windowSecs, point.timestamp)
  return { ...history, [asset]: trimmed }
}

export function applyPriceUpdateToHistory(
  history: PriceHistoryMap,
  prices: Record<string, AssetPriceWire>,
  windowSecs = DEFAULT_HISTORY_WINDOW_SECS,
): PriceHistoryMap {
  let nextHistory = history
  for (const [asset, update] of Object.entries(prices)) {
    nextHistory = appendPricePoint(
      nextHistory,
      asset,
      { timestamp: update.timestamp, price: update.price, source: update.source },
      windowSecs,
    )
  }
  return nextHistory
}
