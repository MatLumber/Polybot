import { useEffect, useMemo, useState } from 'react'
import {
  Activity,
  AlertTriangle,
  BarChart3,
  Bot,
  CandlestickChart,
  Clock3,
  DollarSign,
  Layers,
  ShieldCheck,
  TrendingUp,
  Wallet,
  Wifi,
  WifiOff,
} from 'lucide-react'
import { ChartErrorBoundary } from './charts/ChartErrorBoundary'
import { PriceStreamChart } from './charts/PriceStreamChart'
import { useDashboardStream } from '../hooks/useDashboardStream'
import type { AssetPrice, AssetStats, MarketLearningProgress, Position, Trade } from '../types/ui'

const RECENT_TRADES_WINDOW_MS = 24 * 60 * 60 * 1000

type Mode = 'paper' | 'live'
type ChartAsset = 'BTC' | 'ETH'
type ChartStyle = 'cyber' | 'polymarket'

interface MetricCardProps {
  label: string
  value: string
  sublabel?: string
  tone?: 'default' | 'positive' | 'negative' | 'warning'
}

function MetricCard({ label, value, sublabel, tone = 'default' }: MetricCardProps) {
  return (
    <div className={`metric-card metric-${tone}`}>
      <p className="metric-label">{label}</p>
      <p className="metric-value">{value}</p>
      {sublabel ? <p className="metric-sub">{sublabel}</p> : null}
    </div>
  )
}

function formatCurrency(value: number): string {
  return new Intl.NumberFormat('en-US', {
    style: 'currency',
    currency: 'USD',
    maximumFractionDigits: 2,
  }).format(value)
}

function formatPercent(value: number): string {
  const sign = value >= 0 ? '+' : ''
  return `${sign}${value.toFixed(2)}%`
}

function formatSignedCurrency(value: number): string {
  const sign = value >= 0 ? '+' : ''
  return `${sign}${formatCurrency(value)}`
}

function positionDirectionClass(direction: string): string {
  return direction.toLowerCase() === 'up' ? 'text-positive' : 'text-negative'
}

function pnlClass(value: number): string {
  if (value > 0) return 'text-positive'
  if (value < 0) return 'text-negative'
  return 'text-muted'
}

function formatTimeLeft(totalSeconds: number): string {
  const safe = Math.max(0, Math.floor(totalSeconds))
  const hours = Math.floor(safe / 3600)
  const minutes = Math.floor((safe % 3600) / 60)
  const seconds = safe % 60

  if (hours > 0) {
    return `${hours}h ${minutes}m ${seconds}s`
  }
  return `${minutes}m ${seconds}s`
}

function normalizeTimeframeLabel(raw: string): string {
  const value = raw.trim().toLowerCase()
  if (value === '15m' || value === 'min15' || value === 'm15') return '15M'
  if (value === '1h' || value === 'hour1' || value === 'h1') return '1H'
  return raw.trim().toUpperCase()
}

function formatMarketLabel(asset: string, timeframe: string): string {
  return `${asset.trim().toUpperCase()}_${normalizeTimeframeLabel(timeframe)}`
}

function toMsTimestamp(timestamp: number): number {
  return timestamp < 10_000_000_000 ? timestamp * 1000 : timestamp
}

function buildMarketStatsFromTrades(trades: Trade[]): AssetStats[] {
  const byMarket = new Map<string, { totalTrades: number; wins: number; losses: number; totalPnl: number; confidence: number }>()

  for (const trade of trades) {
    const key = formatMarketLabel(trade.asset, trade.timeframe)
    const current = byMarket.get(key) ?? { totalTrades: 0, wins: 0, losses: 0, totalPnl: 0, confidence: 0 }
    current.totalTrades += 1
    current.totalPnl += trade.pnl
    current.confidence += trade.confidence
    if (trade.pnl >= 0) {
      current.wins += 1
    } else {
      current.losses += 1
    }
    byMarket.set(key, current)
  }

  return Array.from(byMarket.entries()).map(([market, aggregate]) => ({
    asset: market,
    totalTrades: aggregate.totalTrades,
    wins: aggregate.wins,
    losses: aggregate.losses,
    winRate: aggregate.totalTrades > 0 ? (aggregate.wins / aggregate.totalTrades) * 100 : 0,
    totalPnl: aggregate.totalPnl,
    avgConfidence: aggregate.totalTrades > 0 ? aggregate.confidence / aggregate.totalTrades : 0,
  }))
}

function renderPriceChip(asset: string, price?: AssetPrice) {
  if (!price) {
    return (
      <div className="chip" key={asset}>
        <span className="chip-label">{asset}</span>
        <span className="chip-value text-muted">No data</span>
      </div>
    )
  }

  return (
    <div className="chip" key={asset}>
      <span className="chip-label">{asset}</span>
      <span className="chip-value">{formatCurrency(price.price)}</span>
      <span className={`chip-change ${pnlClass(price.change24h)}`}>{formatPercent(price.change24h * 100)}</span>
    </div>
  )
}

function PositionTable({ positions }: { positions: Position[] }) {
  if (positions.length === 0) {
    return <div className="empty-state">No open positions</div>
  }

  return (
    <div className="table-wrap table-wrap-positions">
      <table className="compact-table">
        <thead>
          <tr>
            <th>Market</th>
            <th>Dir</th>
            <th>Entry</th>
            <th>Now</th>
            <th>Size</th>
            <th>PnL</th>
            <th>%</th>
            <th>Time Left</th>
          </tr>
        </thead>
        <tbody>
          {positions.map((position) => (
            <tr key={position.id}>
              <td>{formatMarketLabel(position.asset, position.timeframe)}</td>
              <td className={positionDirectionClass(position.direction)}>{position.direction.toUpperCase()}</td>
              <td>{formatCurrency(position.entryPrice)}</td>
              <td>{formatCurrency(position.currentPrice)}</td>
              <td>{formatCurrency(position.sizeUsdc)}</td>
              <td className={pnlClass(position.pnl)}>{formatSignedCurrency(position.pnl)}</td>
              <td className={pnlClass(position.pnlPct)}>{formatPercent(position.pnlPct)}</td>
              <td>{formatTimeLeft(position.timeRemainingSecs)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function TradeTable({ trades }: { trades: Trade[] }) {
  if (trades.length === 0) {
    return <div className="empty-state">No trades in last 24h</div>
  }

  return (
    <div className="table-wrap table-wrap-trades">
      <table className="compact-table">
        <thead>
          <tr>
            <th>Time</th>
            <th>Market</th>
            <th>Dir</th>
            <th>Entry</th>
            <th>Exit</th>
            <th>PnL</th>
            <th>%</th>
            <th>Reason</th>
          </tr>
        </thead>
        <tbody>
          {trades.map((trade) => (
            <tr key={trade.id}>
              <td>{new Date(toMsTimestamp(trade.timestamp)).toLocaleTimeString()}</td>
              <td>{formatMarketLabel(trade.asset, trade.timeframe)}</td>
              <td className={positionDirectionClass(trade.direction)}>{trade.direction.toUpperCase()}</td>
              <td>{formatCurrency(trade.entryPrice)}</td>
              <td>{formatCurrency(trade.exitPrice)}</td>
              <td className={pnlClass(trade.pnl)}>{formatSignedCurrency(trade.pnl)}</td>
              <td className={pnlClass(trade.pnlPct)}>{formatPercent(trade.pnlPct)}</td>
              <td>{trade.exitReason.replaceAll('_', ' ')}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

function learningStatusLabel(status: MarketLearningProgress['status']): string {
  if (status === 'ready') return 'Listo'
  if (status === 'warming_up') return 'Calentando'
  return 'Sin datos'
}

function learningStatusClass(status: MarketLearningProgress['status']): string {
  if (status === 'ready') return 'learning-ready'
  if (status === 'warming_up') return 'learning-warming'
  return 'learning-idle'
}

function learningSemaphoreClass(status: MarketLearningProgress['status']): string {
  if (status === 'ready') return 'learning-semaphore-green'
  if (status === 'warming_up') return 'learning-semaphore-yellow'
  return 'learning-semaphore-red'
}

function formatLearningTimeframe(timeframe: string): string {
  if (timeframe === '15M') return '15 minutos'
  if (timeframe === '1H') return '1 hora'
  return timeframe
}

function toMsFromSeconds(ts: number): number {
  return ts > 0 && ts < 10_000_000_000 ? ts * 1000 : ts
}

function sortLearningMarkets(items: MarketLearningProgress[]): MarketLearningProgress[] {
  const rank: Record<string, number> = {
    BTC_15M: 1,
    BTC_1H: 2,
    ETH_15M: 3,
    ETH_1H: 4,
  }
  return [...items].sort((a, b) => (rank[a.marketKey] ?? 99) - (rank[b.marketKey] ?? 99))
}

function trainingHint(item: MarketLearningProgress): string {
  if (item.status === 'ready') {
    if (item.avgWinRatePct >= 55) return 'Modelo listo y estable para este mercado.'
    if (item.avgWinRatePct >= 50) return 'Modelo listo; sigue ajustando en tiempo real.'
    return 'Modelo listo, pero con precisión baja por ahora.'
  }

  if (item.sampleCount === 0) {
    return 'Esperando los primeros cierres para empezar a calibrar.'
  }

  const remaining = Math.max(0, item.targetSamples - item.sampleCount)
  return `Faltan ${remaining} cierres para desbloquear métricas avanzadas.`
}

function formatRejectionReason(reason: string): string {
  return reason.replaceAll('_', ' ').replaceAll('-', ' ')
}

function RejectionDiagnosticsPanel({
  processed,
  generated,
  strategyReasons,
  accepted,
  rejected,
  reasons,
  lastReason,
  lastTs,
}: {
  processed: number
  generated: number
  strategyReasons: Record<string, number>
  accepted: number
  rejected: number
  reasons: Record<string, number>
  lastReason: string | null
  lastTs: number
}) {
  const strategyRows = Object.entries(strategyReasons)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
  const rejectionRows = Object.entries(reasons)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 5)
  const maxCount = Math.max(
    ...strategyRows.map(([, c]) => c),
    ...rejectionRows.map(([, c]) => c),
    1
  )

  return (
    <div className="diagnostics-panel">
      <div className="diagnostics-header">
        <div className="diagnostic-stat">
          <span className="diagnostic-stat-label">Features</span>
          <span className="diagnostic-stat-value">{processed}</span>
        </div>
        <div className="diagnostic-stat">
          <span className="diagnostic-stat-label">Signals</span>
          <span className="diagnostic-stat-value">{generated}</span>
        </div>
        <div className="diagnostic-stat">
          <span className="diagnostic-stat-label">Accepted</span>
          <span className="diagnostic-stat-value positive">{accepted}</span>
        </div>
        <div className="diagnostic-stat">
          <span className="diagnostic-stat-label">Rejected</span>
          <span className="diagnostic-stat-value negative">{rejected}</span>
        </div>
      </div>

      <div className="diagnostics-section">
        <div className="diagnostics-section-title">Strategy Filters</div>
        {strategyRows.length === 0 ? (
          <div className="empty-state">No strategy filters yet</div>
        ) : (
          <div className="rejection-bars">
            {strategyRows.map(([reason, count]) => (
              <div className="rejection-bar-item" key={reason}>
                <span className="rejection-bar-label">{formatRejectionReason(reason)}</span>
                <span className="rejection-bar-count">{count}</span>
                <div className="rejection-bar-visual">
                  <div
                    className="rejection-bar-fill"
                    style={{ width: `${(count / maxCount) * 100}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="diagnostics-section">
        <div className="diagnostics-section-title">Execution Rejections</div>
        {rejectionRows.length === 0 ? (
          <div className="empty-state">No execution rejections yet</div>
        ) : (
          <div className="rejection-bars">
            {rejectionRows.map(([reason, count]) => (
              <div className="rejection-bar-item" key={reason}>
                <span className="rejection-bar-label">{formatRejectionReason(reason)}</span>
                <span className="rejection-bar-count">{count}</span>
                <div className="rejection-bar-visual">
                  <div
                    className="rejection-bar-fill"
                    style={{ width: `${(count / maxCount) * 100}%` }}
                  />
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      <div className="last-rejection">
        <div>
          <span className="last-rejection-label">Last Rejection</span>
          <span className="last-rejection-value">
            {lastReason ? formatRejectionReason(lastReason) : 'N/A'}
          </span>
        </div>
        <span className="last-rejection-time">
          {lastTs > 0 ? new Date(lastTs).toLocaleTimeString() : '--:--:--'}
        </span>
      </div>
    </div>
  )
}

function LearningPanel({ items }: { items: MarketLearningProgress[] }) {
  if (items.length === 0) {
    return <div className="empty-state">Waiting for training data...</div>
  }

  const sorted = sortLearningMarkets(items)
  const readyCount = sorted.filter((item) => item.status === 'ready').length
  const totalSamples = sorted.reduce(
    (acc, item) => acc + Math.min(item.sampleCount, item.targetSamples),
    0,
  )
  const totalTargets = sorted.reduce((acc, item) => acc + item.targetSamples, 0)
  const globalProgressPct = totalTargets > 0 ? (totalSamples / totalTargets) * 100 : 0

  return (
    <div className="learning-list">
      <div className="learning-global">
        <div className="learning-global-meta">
          <span>Global Training Progress</span>
          <span>{readyCount}/{sorted.length} markets ready</span>
        </div>
        <div className="learning-bar learning-bar-global">
          <span className="learning-bar-fill" style={{ width: `${globalProgressPct}%` }} />
        </div>
      </div>

      <div className="learning-grid">
        {sorted.map((item) => (
          <div className="learning-row" key={item.marketKey}>
            <div className="learning-headline">
              <div className="learning-title-wrap">
                <span
                  className={`learning-semaphore ${learningSemaphoreClass(item.status)}`}
                  title={`Status: ${learningStatusLabel(item.status)}`}
                />
                <strong>{item.asset}</strong>
                <span>{formatLearningTimeframe(item.timeframe)}</span>
              </div>
              <span className={`learning-status ${learningStatusClass(item.status)}`}>
                {learningStatusLabel(item.status)}
              </span>
            </div>

            {item.status !== 'ready' ? (
              <>
                <div className="learning-meta">
                  <span>{item.sampleCount}/{item.targetSamples} trades</span>
                  <span>{item.progressPct.toFixed(0)}%</span>
                </div>
                <div className="learning-bar">
                  <span className="learning-bar-fill" style={{ width: `${item.progressPct}%` }} />
                </div>
                <p className="learning-hint">{trainingHint(item)}</p>
                <div className="learning-foot">
                  <span>{item.indicatorsActive} indicators</span>
                  <span>
                    Updated{' '}
                    {item.lastUpdatedTs > 0
                      ? new Date(toMsFromSeconds(item.lastUpdatedTs)).toLocaleTimeString()
                      : '--:--:--'}
                  </span>
                </div>
              </>
            ) : (
              <>
                <div className="learning-ready-grid">
                  <div className="learning-ready-kpi">
                    <span>Avg. Accuracy</span>
                    <strong>{item.avgWinRatePct.toFixed(1)}%</strong>
                  </div>
                  <div className="learning-ready-kpi">
                    <span>Indicators</span>
                    <strong>{item.indicatorsActive}</strong>
                  </div>
                  <div className="learning-ready-kpi">
                    <span>Trades</span>
                    <strong>{item.sampleCount}</strong>
                  </div>
                </div>
                <p className="learning-hint">{trainingHint(item)}</p>
                <div className="learning-foot">
                  <span>
                    Updated{' '}
                    {item.lastUpdatedTs > 0
                      ? new Date(toMsFromSeconds(item.lastUpdatedTs)).toLocaleTimeString()
                      : '--:--:--'}
                  </span>
                </div>
              </>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}

export function Dashboard() {
  const stream = useDashboardStream()
  const [mode, setMode] = useState<Mode>('paper')
  const [chartAsset, setChartAsset] = useState<ChartAsset>('BTC')
  const [chartStyle, setChartStyle] = useState<ChartStyle>('cyber')
  const [heartbeatNow, setHeartbeatNow] = useState(() => Date.now())
  const dashboard = stream.dashboard

  useEffect(() => {
    const timer = window.setInterval(() => {
      setHeartbeatNow(Date.now())
    }, 1000)
    return () => window.clearInterval(timer)
  }, [])

  const staleHeartbeat =
    stream.lastHeartbeatAt !== null && heartbeatNow - stream.lastHeartbeatAt > 20_000
  const connectionLabel = stream.connected ? (staleHeartbeat ? 'Stale' : 'Live') : 'Offline'
  const connectionClass = stream.connected ? (staleHeartbeat ? 'status-stale' : 'status-live') : 'status-offline'

  const modeState = useMemo(() => {
    if (!dashboard) return null
    return mode === 'paper' ? dashboard.paper : dashboard.live
  }, [dashboard, mode])

  if (stream.error && !dashboard) {
    return (
      <div className="dashboard-shell">
        <div className="center-card">
          <AlertTriangle size={18} />
          <div>
            <h2>Connection error</h2>
            <p>{stream.error}</p>
          </div>
        </div>
      </div>
    )
  }

  if (!dashboard || !modeState) {
    return (
      <div className="dashboard-shell">
        <div className="center-card">
          <Activity className="spin" size={18} />
          <div>
            <h2>Syncing dashboard</h2>
            <p>Loading state, prices, and stream...</p>
          </div>
        </div>
      </div>
    )
  }

  const positions = modeState.openPositions
  const trades = dashboard.paper.recentTrades
    .filter((trade) => toMsTimestamp(trade.timestamp) >= heartbeatNow - RECENT_TRADES_WINDOW_MS)
    .sort((a, b) => toMsTimestamp(b.timestamp) - toMsTimestamp(a.timestamp))
    .slice(0, 40)
  const stats = dashboard.paper.stats
  const priceMap = dashboard.prices.prices
  const execution = dashboard.execution
  const marketLearning = stream.marketLearning
  const selectedChartPrice = priceMap[chartAsset]
  const chartWindowSeconds = chartStyle === 'polymarket' ? 86_400 : 3600
  const chartPeriodLabel = chartStyle === 'polymarket' ? '15m · 24h' : '1h'
  const assetStats = buildMarketStatsFromTrades(trades)
    .sort((a, b) => b.totalPnl - a.totalPnl)
    .slice(0, 8)

  return (
    <div className="dashboard-shell">
      <div className="fx-grid" />
      <header className="topbar glass-panel">
        <div className="brand">
          <div className="brand-icon">
            <Bot size={14} />
          </div>
          <div>
            <h1>PolyBot V2</h1>
            <p>Cyber Trading Command</p>
          </div>
        </div>

        <div className="topbar-controls">
          <div className={`status-pill ${connectionClass}`}>
            {stream.connected ? <Wifi size={12} /> : <WifiOff size={12} />}
            <span>{connectionLabel}</span>
          </div>
          <div className="toggle">
            <button
              className={mode === 'paper' ? 'active' : ''}
              onClick={() => setMode('paper')}
              type="button"
            >
              Paper
            </button>
            <button
              className={mode === 'live' ? 'active' : ''}
              onClick={() => setMode('live')}
              type="button"
            >
              Live
            </button>
          </div>
        </div>
      </header>

      <main className="dashboard-main">
        <section className="glass-panel chart-panel">
          <div className="panel-head">
            <div className="panel-title">
              <CandlestickChart size={14} />
              <span>{chartAsset} {chartStyle === 'polymarket' ? 'Candles' : 'Trend'} ({chartPeriodLabel})</span>
              <span className={`chip-change ${pnlClass(selectedChartPrice?.change24h ?? 0)}`}>
                {selectedChartPrice ? formatPercent(selectedChartPrice.change24h * 100) : '--'}
              </span>
            </div>
            <div className="chart-head-controls">
              <div className="chart-switch">
                <button
                  className={chartStyle === 'cyber' ? 'active' : ''}
                  onClick={() => setChartStyle('cyber')}
                  type="button"
                >
                  Cyber
                </button>
                <button
                  className={chartStyle === 'polymarket' ? 'active' : ''}
                  onClick={() => setChartStyle('polymarket')}
                  type="button"
                >
                  Polymarket
                </button>
              </div>
              <div className="chart-switch">
                <button
                  className={chartAsset === 'BTC' ? 'active' : ''}
                  onClick={() => setChartAsset('BTC')}
                  type="button"
                >
                  BTC
                </button>
                <button
                  className={chartAsset === 'ETH' ? 'active' : ''}
                  onClick={() => setChartAsset('ETH')}
                  type="button"
                >
                  ETH
                </button>
              </div>
              <div className="chip-row">
                {renderPriceChip('BTC', priceMap.BTC)}
                {renderPriceChip('ETH', priceMap.ETH)}
              </div>
            </div>
          </div>
          <ChartErrorBoundary>
            <PriceStreamChart
              history={stream.priceHistory}
              livePrices={priceMap}
              selectedAsset={chartAsset}
              chartStyle={chartStyle}
              windowSeconds={chartWindowSeconds}
            />
          </ChartErrorBoundary>
        </section>

        <section className="metrics-grid">
          <MetricCard
            label="Total Equity"
            value={formatCurrency(modeState.totalEquity)}
            sublabel={`Balance ${formatCurrency(modeState.balance)}`}
          />
          <MetricCard
            label="Available"
            value={formatCurrency(modeState.available)}
            sublabel={`Locked ${formatCurrency(modeState.locked)}`}
          />
          <MetricCard
            label="Unrealized"
            value={formatSignedCurrency(modeState.unrealizedPnl)}
            tone={modeState.unrealizedPnl >= 0 ? 'positive' : 'negative'}
          />
          <MetricCard
            label={mode === 'paper' ? 'Win Rate' : 'Daily PnL'}
            value={mode === 'paper' ? `${stats.winRate.toFixed(1)}%` : formatSignedCurrency(dashboard.live.dailyPnl)}
            tone={mode === 'paper' ? (stats.winRate >= 50 ? 'positive' : 'warning') : dashboard.live.dailyPnl >= 0 ? 'positive' : 'negative'}
          />
          <MetricCard
            label="Drawdown"
            value={`${stats.currentDrawdown.toFixed(2)}%`}
            sublabel={`Max ${stats.maxDrawdown.toFixed(2)}%`}
            tone="warning"
          />
          <MetricCard
            label="Profit Factor"
            value={Number.isFinite(stats.profitFactor) ? stats.profitFactor.toFixed(2) : 'INF'}
          />
          <MetricCard
            label="Streak"
            value={`${stats.currentStreak > 0 ? '+' : ''}${stats.currentStreak}`}
            sublabel={`Best +${stats.bestStreak} | Worst ${stats.worstStreak}`}
            tone={stats.currentStreak >= 0 ? 'positive' : 'negative'}
          />
          <MetricCard label="Open Positions" value={`${positions.length}`} />
        </section>

        <section className="panels-grid">
          <div className="glass-panel">
            <div className="panel-head">
              <div className="panel-title">
                <Layers size={14} />
                <span>{mode === 'paper' ? 'Paper' : 'Live'} Positions</span>
              </div>
            </div>
            <PositionTable positions={positions} />
          </div>

          <div className="glass-panel">
            <div className="panel-head">
              <div className="panel-title">
                <BarChart3 size={14} />
                <span>Recent Trades (24h) · {trades.length}</span>
              </div>
            </div>
            <TradeTable trades={trades} />
          </div>

          <div className="glass-panel">
            <div className="panel-head">
              <div className="panel-title">
                <TrendingUp size={14} />
                <span>Market Performance (24h)</span>
              </div>
            </div>
            {assetStats.length === 0 ? (
              <div className="empty-state">No market stats in last 24h</div>
            ) : (
              <div className="asset-list">
                {assetStats.map((asset) => (
                  <div className="asset-row" key={asset.asset}>
                    <div className="asset-name">
                      <strong>{asset.asset}</strong>
                      <span>{asset.totalTrades} trades</span>
                    </div>
                    <div className="asset-values">
                      <span className={pnlClass(asset.totalPnl)}>{formatSignedCurrency(asset.totalPnl)}</span>
                      <span>{asset.winRate.toFixed(1)}% WR</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          <div className="glass-panel">
            <div className="panel-head">
              <div className="panel-title">
                <AlertTriangle size={14} />
                <span>Signal Diagnostics</span>
              </div>
            </div>
            <RejectionDiagnosticsPanel
              processed={execution.processedFeatures}
              generated={execution.generatedSignals}
              strategyReasons={execution.strategyFilterReasons}
              accepted={execution.acceptedSignals}
              rejected={execution.rejectedSignals}
              reasons={execution.rejectionReasons}
              lastReason={execution.lastRejectionReason}
              lastTs={execution.lastRejectionTs}
            />
          </div>
        </section>

        <section className="glass-panel learning-panel">
          <div className="panel-head">
            <div className="panel-title">
              <Activity size={14} />
              <span>Market Training</span>
            </div>
          </div>
          <p className="learning-help">
            Each market trains independently. Target: 30 closed trades per market for full calibration.
          </p>
          <LearningPanel items={marketLearning} />
        </section>
      </main>

      <footer className="footer-bar">
        <span className="footer-item">
          <Clock3 size={12} />
          Last update {new Date(dashboard.timestamp).toLocaleTimeString()}
        </span>
        <span className="footer-item">
          <Wallet size={12} />
          Equity {formatCurrency(modeState.totalEquity)}
        </span>
        <span className="footer-item">
          <DollarSign size={12} />
          Fees {formatCurrency(stats.totalFees)}
        </span>
        <span className="footer-item">
          <ShieldCheck size={12} />
          Kill switch {dashboard.live.killSwitchActive ? 'ON' : 'OFF'}
        </span>
      </footer>
    </div>
  )
}

