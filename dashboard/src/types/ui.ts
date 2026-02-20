export interface PaperStats {
  totalTrades: number
  wins: number
  losses: number
  winRate: number
  totalPnl: number
  totalFees: number
  largestWin: number
  largestLoss: number
  avgWin: number
  avgLoss: number
  maxDrawdown: number
  currentDrawdown: number
  peakBalance: number
  profitFactor: number
  currentStreak: number
  bestStreak: number
  worstStreak: number
  exitsTrailingStop: number
  exitsTakeProfit: number
  exitsMarketExpiry: number
  exitsTimeExpiry: number
}

export interface Position {
  id: string
  asset: string
  timeframe: string
  direction: string
  entryPrice: number
  currentPrice: number
  sizeUsdc: number
  pnl: number
  pnlPct: number
  openedAt: number
  marketSlug: string
  confidence: number
  peakPrice: number
  troughPrice: number
  marketCloseTs: number
  timeRemainingSecs: number
}

export interface Trade {
  id: string
  timestamp: number
  asset: string
  timeframe: string
  direction: string
  confidence: number
  entryPrice: number
  exitPrice: number
  sizeUsdc: number
  pnl: number
  pnlPct: number
  result: string
  exitReason: string
  holdDurationSecs: number
  balanceAfter: number
}

export interface AssetStats {
  asset: string
  totalTrades: number
  wins: number
  losses: number
  winRate: number
  totalPnl: number
  avgConfidence: number
}

export interface AssetPrice {
  asset: string
  price: number
  bid: number
  ask: number
  source: string
  timestamp: number
  change24h: number
}

export interface PriceHistoryPoint {
  timestamp: number
  price: number
  source: string
}

export type MarketLearningStatus = 'idle' | 'warming_up' | 'ready'

export interface MarketLearningProgress {
  marketKey: string
  asset: string
  timeframe: string
  sampleCount: number
  targetSamples: number
  progressPct: number
  indicatorsActive: number
  avgWinRatePct: number
  lastUpdatedTs: number
  status: MarketLearningStatus
}

export interface PaperDashboard {
  balance: number
  available: number
  locked: number
  totalEquity: number
  unrealizedPnl: number
  stats: PaperStats
  openPositions: Position[]
  recentTrades: Trade[]
  assetStats: Record<string, AssetStats>
}

export interface LiveDashboard {
  balance: number
  available: number
  locked: number
  totalEquity: number
  unrealizedPnl: number
  openPositions: Position[]
  dailyPnl: number
  dailyTrades: number
  killSwitchActive: boolean
}

export interface PriceDashboard {
  prices: Record<string, AssetPrice>
  lastUpdate: number
}

export interface ExecutionDiagnostics {
  processedFeatures: number
  generatedSignals: number
  filteredFeatures: number
  strategyFilterReasons: Record<string, number>
  lastStrategyFilterReason: string | null
  lastStrategyFilterTs: number
  acceptedSignals: number
  rejectedSignals: number
  rejectionReasons: Record<string, number>
  lastRejectionReason: string | null
  lastRejectionTs: number
}

export interface DashboardState {
  paper: PaperDashboard
  live: LiveDashboard
  prices: PriceDashboard
  execution: ExecutionDiagnostics
  timestamp: number
}

export type PriceHistoryMap = Record<string, PriceHistoryPoint[]>

export interface DashboardStreamState {
  dashboard: DashboardState | null
  priceHistory: PriceHistoryMap
  marketLearning: MarketLearningProgress[]
  mlState: MLState | null
  mlMetrics: MLMetrics | null
  mlPrediction: MLPrediction | null
  connected: boolean
  status: 'connecting' | 'connected' | 'disconnected' | 'error'
  error: string | null
  lastHeartbeatAt: number | null
  lastMessageAt: number | null
}

// ML Types
export interface MLModelInfo {
  name: string
  weight: number
  accuracy: number
  status: string
}

export interface MLState {
  enabled: boolean
  modelType: string
  version: string
  timestamp: number
}

export interface MLMetrics {
  accuracy: number
  winRate: number
  lossRate: number
  totalPredictions: number
  correctPredictions: number
  incorrectPredictions: number
  ensembleWeights: MLModelInfo[]
  epoch: number
  datasetSize: number
  timestamp: number
}

export interface MLPrediction {
  asset: string
  timeframe: string
  direction: string
  confidence: number
  probUp: number
  modelName: string
  featuresTriggered: string[]
  timestamp: number
}

export interface MLFeatures {
  totalFeatures: number
  topFeatures: Array<{ name: string; importance: number }>
  timestamp: number
}

export interface MLTraining {
  status: string
  lastTraining: number | null
  samplesTrained: number
  retrainInterval: number
  walkForwardEnabled: boolean
  timestamp: number
}
