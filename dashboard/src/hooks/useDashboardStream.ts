import { useCallback, useEffect, useMemo, useReducer, useRef } from 'react'
import {
  applyPriceUpdateToHistory,
  mapDashboardState,
  mapMarketLearningProgressList,
  mapMLMetrics,
  mapMLPrediction,
  mapMLState,
  mapPaperStats,
  mapPosition,
  mapPriceHistory,
  mapPriceMap,
  mapTrade,
} from '../lib/mappers'
import {
  buildApiCandidates,
  buildWsCandidates,
  DEFAULT_API_BASE,
  DEFAULT_WS_URL,
  fetchApiFromCandidates,
  normalizeBaseUrl,
} from '../lib/runtimeEndpoints'
import type { DashboardStreamState, Trade } from '../types/ui'
import type {
  DashboardStateWire,
  MarketLearningProgressWire,
  MLMetricsWire,
  MLStateWire,
  PriceHistoryPointWire,
  WsMessageWire,
} from '../types/wire'

const MAX_RECENT_TRADES = 100

type StreamAction =
  | {
    type: 'BOOTSTRAP_SUCCESS'
    dashboard: DashboardStateWire
    history: Record<string, PriceHistoryPointWire[]>
    marketLearning: MarketLearningProgressWire[]
    mlState: MLStateWire
    mlMetrics: MLMetricsWire
  }
  | { type: 'MARKET_LEARNING_UPDATE'; marketLearning: MarketLearningProgressWire[] }
  | { type: 'WS_CONNECTED' }
  | { type: 'WS_DISCONNECTED' }
  | { type: 'WS_ERROR'; error: string }
  | { type: 'WS_MESSAGE'; message: WsMessageWire; receivedAt: number }

const initialState: DashboardStreamState = {
  dashboard: null,
  priceHistory: {},
  marketLearning: [],
  mlState: null,
  mlMetrics: null,
  mlPrediction: null,
  connected: false,
  status: 'connecting',
  error: null,
  lastHeartbeatAt: null,
  lastMessageAt: null,
}

function mergeRecentTrades(current: Trade[], incoming: Trade): Trade[] {
  const deduped = [incoming, ...current.filter((trade) => trade.id !== incoming.id)]
  return deduped.slice(0, MAX_RECENT_TRADES)
}

function reducer(state: DashboardStreamState, action: StreamAction): DashboardStreamState {
  switch (action.type) {
    case 'BOOTSTRAP_SUCCESS':
      return {
        ...state,
        dashboard: mapDashboardState(action.dashboard),
        priceHistory: mapPriceHistory(action.history),
        marketLearning: mapMarketLearningProgressList(action.marketLearning),
        mlState: mapMLState(action.mlState),
        mlMetrics: mapMLMetrics(action.mlMetrics),
        status: state.connected ? 'connected' : 'connecting',
        error: null,
      }
    case 'MARKET_LEARNING_UPDATE':
      return {
        ...state,
        marketLearning: mapMarketLearningProgressList(action.marketLearning),
      }
    case 'WS_CONNECTED':
      return {
        ...state,
        connected: true,
        status: 'connected',
        error: null,
      }
    case 'WS_DISCONNECTED':
      return {
        ...state,
        connected: false,
        status: 'disconnected',
      }
    case 'WS_ERROR':
      return {
        ...state,
        connected: false,
        status: 'error',
        error: action.error,
      }
    case 'WS_MESSAGE': {
      const { message, receivedAt } = action
      const nextState: DashboardStreamState = {
        ...state,
        lastMessageAt: receivedAt,
      }

      if (message.type === 'Heartbeat') {
        return {
          ...nextState,
          lastHeartbeatAt: message.data,
        }
      }

      if (message.type === 'FullState') {
        return {
          ...nextState,
          dashboard: mapDashboardState(message.data),
          error: null,
        }
      }

      if (!state.dashboard) {
        return nextState
      }

      if (message.type === 'StatsUpdate') {
        return {
          ...nextState,
          dashboard: {
            ...state.dashboard,
            paper: {
              ...state.dashboard.paper,
              stats: mapPaperStats(message.data),
            },
          },
        }
      }

      if (message.type === 'PriceUpdate') {
        const mappedPrices = mapPriceMap(message.data)
        const lastUpdate = Object.values(mappedPrices).reduce(
          (maxTs, price) => Math.max(maxTs, price.timestamp),
          state.dashboard.prices.lastUpdate,
        )
        return {
          ...nextState,
          dashboard: {
            ...state.dashboard,
            prices: {
              prices: { ...state.dashboard.prices.prices, ...mappedPrices },
              lastUpdate,
            },
          },
          priceHistory: applyPriceUpdateToHistory(state.priceHistory, message.data),
        }
      }

      if (message.type === 'NewTrade') {
        const trade = mapTrade(message.data)
        return {
          ...nextState,
          dashboard: {
            ...state.dashboard,
            paper: {
              ...state.dashboard.paper,
              recentTrades: mergeRecentTrades(state.dashboard.paper.recentTrades, trade),
            },
          },
        }
      }

      if (message.type === 'PositionOpened') {
        const opened = mapPosition(message.data)
        const withoutDuplicate = state.dashboard.paper.openPositions.filter(
          (position) => position.id !== opened.id,
        )
        return {
          ...nextState,
          dashboard: {
            ...state.dashboard,
            paper: {
              ...state.dashboard.paper,
              openPositions: [...withoutDuplicate, opened],
              balance: state.dashboard.paper.balance - opened.sizeUsdc,
              locked: state.dashboard.paper.locked + opened.sizeUsdc,
            },
          },
        }
      }

      if (message.type === 'PositionClosed') {
        const closedTrade = mapTrade(message.data.trade)
        return {
          ...nextState,
          dashboard: {
            ...state.dashboard,
            paper: {
              ...state.dashboard.paper,
              openPositions: state.dashboard.paper.openPositions.filter(
                (position) => position.id !== message.data.position_id,
              ),
              recentTrades: mergeRecentTrades(state.dashboard.paper.recentTrades, closedTrade),
              balance: state.dashboard.paper.balance + closedTrade.sizeUsdc + closedTrade.pnl,
              locked: Math.max(0, state.dashboard.paper.locked - closedTrade.sizeUsdc),
            },
          },
        }
      }

      if (message.type === 'PositionsUpdate') {
        return {
          ...nextState,
          dashboard: {
            ...state.dashboard,
            paper: {
              ...state.dashboard.paper,
              openPositions: message.data.map(mapPosition),
            },
          },
        }
      }

      if (message.type === 'LivePositionsUpdate') {
        return {
          ...nextState,
          dashboard: {
            ...state.dashboard,
            live: {
              ...state.dashboard.live,
              openPositions: message.data.map(mapPosition),
            },
          },
        }
      }

      if (message.type === 'LiveBalanceUpdate') {
        return {
          ...nextState,
          dashboard: {
            ...state.dashboard,
            live: {
              ...state.dashboard.live,
              balance: message.data.balance,
              locked: message.data.locked,
              available: message.data.balance - message.data.locked,
            },
          },
        }
      }

      if (message.type === 'LiveStatsUpdate') {
        return {
          ...nextState,
          dashboard: {
            ...state.dashboard,
            live: {
              ...state.dashboard.live,
              stats: mapPaperStats(message.data), // LiveStatsResponse has identical schema to PaperStatsResponse
            },
          },
        }
      }

      // ML Messages
      if (message.type === 'MLStateUpdate') {
        return {
          ...nextState,
          mlState: mapMLState(message.data),
        }
      }

      if (message.type === 'MLMetricsUpdate') {
        return {
          ...nextState,
          mlMetrics: mapMLMetrics(message.data),
        }
      }

      if (message.type === 'MLPrediction') {
        return {
          ...nextState,
          mlPrediction: mapMLPrediction(message.data),
        }
      }

      return nextState
    }
    default:
      return state
  }
}

export function useDashboardStream() {
  const [state, dispatch] = useReducer(reducer, initialState)
  const wsRef = useRef<WebSocket | null>(null)
  const reconnectTimerRef = useRef<number | null>(null)
  const reconnectAttemptRef = useRef(0)
  const wsCandidateIndexRef = useRef(0)
  const mountedRef = useRef(false)
  const connectRef = useRef<() => void>(() => { })

  const apiBase = useMemo(
    () => normalizeBaseUrl(import.meta.env.VITE_API_BASE ?? DEFAULT_API_BASE),
    [],
  )
  const apiCandidates = useMemo(() => buildApiCandidates(apiBase), [apiBase])
  const wsCandidates = useMemo(
    () => buildWsCandidates(import.meta.env.VITE_WS_URL, apiBase),
    [apiBase],
  )

  const bootstrap = useCallback(async () => {
    const [dashboard, history, marketLearning, mlState, mlMetrics] = await Promise.all([
      fetchApiFromCandidates<DashboardStateWire>('/api/stats', apiCandidates),
      fetchApiFromCandidates<Record<string, PriceHistoryPointWire[]>>(
        '/api/prices/history?assets=BTC,ETH&window_secs=86400&bucket_ms=1000',
        apiCandidates,
      ),
      fetchApiFromCandidates<MarketLearningProgressWire[]>('/api/calibration/markets', apiCandidates),
      fetchApiFromCandidates<MLStateWire>('/api/ml/state', apiCandidates),
      fetchApiFromCandidates<MLMetricsWire>('/api/ml/metrics', apiCandidates),
    ])

    if (!mountedRef.current) {
      return
    }

    dispatch({
      type: 'BOOTSTRAP_SUCCESS',
      dashboard: dashboard.data,
      history: history.data,
      marketLearning: marketLearning.data,
      mlState: mlState.data,
      mlMetrics: mlMetrics.data,
    })
  }, [apiCandidates])

  const refreshMarketLearning = useCallback(async () => {
    const marketLearning = await fetchApiFromCandidates<MarketLearningProgressWire[]>(
      '/api/calibration/markets',
      apiCandidates,
    )

    if (!mountedRef.current) {
      return
    }

    dispatch({ type: 'MARKET_LEARNING_UPDATE', marketLearning: marketLearning.data })
  }, [apiCandidates])

  const scheduleReconnect = useCallback(() => {
    if (!mountedRef.current || reconnectTimerRef.current !== null) {
      return
    }

    reconnectAttemptRef.current += 1
    if (wsCandidates.length > 1) {
      wsCandidateIndexRef.current = (wsCandidateIndexRef.current + 1) % wsCandidates.length
    }
    const expDelay = Math.min(10_000, 500 * 2 ** (reconnectAttemptRef.current - 1))
    reconnectTimerRef.current = window.setTimeout(() => {
      reconnectTimerRef.current = null
      if (!mountedRef.current) {
        return
      }
      connectRef.current()
    }, expDelay)
  }, [wsCandidates.length])

  const connect = useCallback(() => {
    if (!mountedRef.current) {
      return
    }
    const wsUrl = wsCandidates[wsCandidateIndexRef.current] ?? DEFAULT_WS_URL
    if (wsRef.current && (wsRef.current.readyState === WebSocket.OPEN || wsRef.current.readyState === WebSocket.CONNECTING)) {
      return
    }

    const ws = new WebSocket(wsUrl)
    wsRef.current = ws

    ws.onopen = () => {
      reconnectAttemptRef.current = 0
      dispatch({ type: 'WS_CONNECTED' })
      void bootstrap().catch((error: unknown) => {
        dispatch({
          type: 'WS_ERROR',
          error: error instanceof Error ? error.message : 'Failed to resync dashboard after connect',
        })
      })
    }

    ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data) as WsMessageWire
        dispatch({ type: 'WS_MESSAGE', message, receivedAt: Date.now() })
      } catch {
        // Ignore malformed frames; next good frame will re-sync.
      }
    }

    ws.onerror = () => {
      dispatch({ type: 'WS_ERROR', error: `WebSocket connection error (${wsUrl})` })
      ws.close()
    }

    ws.onclose = () => {
      dispatch({ type: 'WS_DISCONNECTED' })
      scheduleReconnect()
    }
  }, [bootstrap, scheduleReconnect, wsCandidates])

  useEffect(() => {
    connectRef.current = connect
  }, [connect])

  useEffect(() => {
    mountedRef.current = true

    void bootstrap().catch((error: unknown) => {
      dispatch({
        type: 'WS_ERROR',
        error: error instanceof Error ? error.message : 'Failed to bootstrap dashboard data',
      })
    })

    connect()
    const marketLearningTimer = window.setInterval(() => {
      void refreshMarketLearning().catch(() => {
        // Ignore transient errors; next poll will retry.
      })
    }, 10_000)

    return () => {
      mountedRef.current = false
      clearInterval(marketLearningTimer)
      if (reconnectTimerRef.current !== null) {
        clearTimeout(reconnectTimerRef.current)
      }
      wsRef.current?.close()
      wsRef.current = null
    }
  }, [bootstrap, connect, refreshMarketLearning])

  return state
}
