import { useEffect, useMemo, useRef } from 'react'
import {
  AreaSeries,
  CandlestickSeries,
  ColorType,
  createChart,
  LineStyle,
  type IChartApi,
  type ISeriesApi,
  type Time,
  type UTCTimestamp,
} from 'lightweight-charts'
import type { AssetPrice, PriceHistoryMap } from '../../types/ui'

type TrackedAsset = 'BTC' | 'ETH'
type ChartStyle = 'cyber' | 'polymarket'

interface PriceStreamChartProps {
  history: PriceHistoryMap
  livePrices: Record<string, AssetPrice>
  selectedAsset: TrackedAsset
  chartStyle: ChartStyle
  windowSeconds?: number
}

type LinePoint = { time: UTCTimestamp; value: number }
type CandlePoint = {
  time: UTCTimestamp
  open: number
  high: number
  low: number
  close: number
}
type HistorySignature = {
  key: string
  first: UTCTimestamp | null
  last: UTCTimestamp | null
  len: number
}

const MIN_CANDLE_BUCKET_SECS = 30
const MAX_CANDLE_BUCKET_SECS = 300
const TARGET_CANDLE_COUNT = 36
const POLYMARKET_CANDLE_BUCKET_SECS = 900
const POLYMARKET_MAX_VISIBLE_BARS = 48
const CYBER_VISUAL_STEP_MS = 1000
const POLYMARKET_VISUAL_STEP_MS = 1000

function toChartTime(timestamp: number): UTCTimestamp {
  return Math.floor(timestamp / 1000) as UTCTimestamp
}

function normalizeTimestampMs(timestamp: number): number {
  return timestamp < 10_000_000_000 ? timestamp * 1000 : timestamp
}

function normalizeLinePoints(points: PriceHistoryMap[TrackedAsset], cutoffMs: number): LinePoint[] {
  const sorted = [...(points ?? [])]
    .filter((point) => normalizeTimestampMs(point.timestamp) >= cutoffMs)
    .sort((a, b) => normalizeTimestampMs(a.timestamp) - normalizeTimestampMs(b.timestamp))

  const line: LinePoint[] = []
  for (const point of sorted) {
    const time = toChartTime(normalizeTimestampMs(point.timestamp))
    const last = line[line.length - 1]
    if (last && last.time === time) {
      line[line.length - 1] = { time, value: point.price }
      continue
    }
    if (!last || time > last.time) {
      line.push({ time, value: point.price })
    }
  }
  return line
}

function buildCandles(lineData: LinePoint[], bucketSecs: number): CandlePoint[] {
  const candles: CandlePoint[] = []
  for (const point of lineData) {
    const bucketTime = (Math.floor(Number(point.time) / bucketSecs) * bucketSecs) as UTCTimestamp
    const last = candles[candles.length - 1]
    if (!last || bucketTime > last.time) {
      candles.push({
        time: bucketTime,
        open: point.value,
        high: point.value,
        low: point.value,
        close: point.value,
      })
      continue
    }
    candles[candles.length - 1] = {
      time: last.time,
      open: last.open,
      high: Math.max(last.high, point.value),
      low: Math.min(last.low, point.value),
      close: point.value,
    }
  }
  return candles
}

function resolveCandleBucketSecs(lineData: LinePoint[]): number {
  if (lineData.length < 2) {
    return 60
  }

  const first = Number(lineData[0].time)
  const last = Number(lineData[lineData.length - 1].time)
  const spanSecs = Math.max(60, last - first)
  const raw = Math.round(spanSecs / TARGET_CANDLE_COUNT)
  return Math.max(MIN_CANDLE_BUCKET_SECS, Math.min(MAX_CANDLE_BUCKET_SECS, raw))
}

export function PriceStreamChart({
  history,
  livePrices,
  selectedAsset,
  chartStyle,
  windowSeconds = 3600,
}: PriceStreamChartProps) {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const chartRef = useRef<IChartApi | null>(null)
  const areaSeriesRef = useRef<ISeriesApi<'Area', Time> | null>(null)
  const candleSeriesRef = useRef<ISeriesApi<'Candlestick', Time> | null>(null)
  const lastLineTimeRef = useRef<UTCTimestamp | null>(null)
  const lastCandleRef = useRef<CandlePoint | null>(null)
  const candleBucketSecsRef = useRef<number>(POLYMARKET_CANDLE_BUCKET_SECS)
  const lastVisualUpdateMsRef = useRef<number>(0)
  const historySignatureRef = useRef<HistorySignature | null>(null)

  const selectedLivePrice = livePrices[selectedAsset]
  const isRising = (selectedLivePrice?.change24h ?? 0) >= 0

  const trendColors = useMemo(
    () =>
      isRising
        ? {
            lineColor: '#7efb9a',
            topColor: 'rgba(126, 251, 154, 0.32)',
            bottomColor: 'rgba(126, 251, 154, 0.03)',
            priceLineColor: '#7efb9a',
          }
        : {
            lineColor: '#ff6a8f',
            topColor: 'rgba(255, 106, 143, 0.28)',
            bottomColor: 'rgba(255, 106, 143, 0.03)',
            priceLineColor: '#ff6a8f',
          },
    [isRising],
  )

  const chartTheme = useMemo(
    () =>
      chartStyle === 'polymarket'
        ? {
            layout: {
              background: { type: ColorType.Solid, color: 'rgba(9, 13, 21, 0.34)' },
              textColor: 'rgba(208, 216, 230, 0.9)',
            },
            grid: {
              vertLines: { color: 'rgba(55, 68, 89, 0.35)' },
              horzLines: { color: 'rgba(55, 68, 89, 0.25)' },
            },
            rightPriceScale: { borderColor: 'rgba(67, 83, 107, 0.6)' },
            timeScale: {
              borderColor: 'rgba(67, 83, 107, 0.6)',
              timeVisible: true,
              secondsVisible: false,
              barSpacing: 11,
              minBarSpacing: 8,
              rightOffset: 3,
            },
            crosshair: {
              vertLine: { color: 'rgba(139, 155, 182, 0.5)' },
              horzLine: { color: 'rgba(139, 155, 182, 0.5)' },
            },
          }
        : {
            layout: {
              background: { type: ColorType.Solid, color: 'rgba(7, 12, 24, 0.25)' },
              textColor: 'rgba(214, 232, 255, 0.9)',
            },
            grid: {
              vertLines: { color: 'rgba(21, 44, 84, 0.5)' },
              horzLines: { color: 'rgba(21, 44, 84, 0.35)' },
            },
            rightPriceScale: { borderColor: 'rgba(49, 84, 138, 0.6)' },
            timeScale: {
              borderColor: 'rgba(49, 84, 138, 0.6)',
              timeVisible: true,
              secondsVisible: false,
            },
            crosshair: {
              vertLine: { color: 'rgba(137, 255, 250, 0.35)' },
              horzLine: { color: 'rgba(137, 255, 250, 0.35)' },
            },
          },
    [chartStyle],
  )

  useEffect(() => {
    if (!containerRef.current || chartRef.current) {
      return
    }

    const chart = createChart(containerRef.current, {
      autoSize: true,
      layout: {
        background: { type: ColorType.Solid, color: 'rgba(7, 12, 24, 0.25)' },
        textColor: 'rgba(214, 232, 255, 0.9)',
      },
      grid: {
        vertLines: { color: 'rgba(21, 44, 84, 0.5)' },
        horzLines: { color: 'rgba(21, 44, 84, 0.35)' },
      },
      rightPriceScale: { borderColor: 'rgba(49, 84, 138, 0.6)' },
      timeScale: {
        borderColor: 'rgba(49, 84, 138, 0.6)',
        timeVisible: true,
        secondsVisible: false,
      },
      crosshair: {
        vertLine: { color: 'rgba(137, 255, 250, 0.35)' },
        horzLine: { color: 'rgba(137, 255, 250, 0.35)' },
      },
    })

    const area = chart.addSeries(AreaSeries, {
      lineWidth: 2,
      lineStyle: LineStyle.Solid,
      lastValueVisible: true,
      crosshairMarkerVisible: true,
      crosshairMarkerRadius: 4,
      lineColor: '#7af2ff',
      topColor: 'rgba(122, 242, 255, 0.28)',
      bottomColor: 'rgba(122, 242, 255, 0.03)',
      priceLineColor: '#7af2ff',
      visible: true,
    })
    const candles = chart.addSeries(CandlestickSeries, {
      upColor: '#16c784',
      downColor: '#ea3943',
      wickUpColor: '#16c784',
      wickDownColor: '#ea3943',
      borderUpColor: '#16c784',
      borderDownColor: '#ea3943',
      borderVisible: true,
      visible: false,
      lastValueVisible: true,
    })

    chartRef.current = chart
    areaSeriesRef.current = area
    candleSeriesRef.current = candles

    const observer = new ResizeObserver(() => {
      chart.timeScale().scrollToRealTime()
    })
    observer.observe(containerRef.current)

    return () => {
      observer.disconnect()
      chart.remove()
      chartRef.current = null
      areaSeriesRef.current = null
      candleSeriesRef.current = null
      lastLineTimeRef.current = null
      lastCandleRef.current = null
      historySignatureRef.current = null
    }
  }, [])

  useEffect(() => {
    const chart = chartRef.current
    const area = areaSeriesRef.current
    const candles = candleSeriesRef.current
    if (!chart || !area || !candles) {
      return
    }

    chart.applyOptions(chartTheme)
    area.applyOptions({
      title: selectedAsset,
      visible: chartStyle === 'cyber',
      ...trendColors,
    })
    candles.applyOptions({
      title: selectedAsset,
      visible: chartStyle === 'polymarket',
    })
  }, [chartStyle, chartTheme, selectedAsset, trendColors])

  useEffect(() => {
    const chart = chartRef.current
    const area = areaSeriesRef.current
    const candles = candleSeriesRef.current
    if (!chart || !area || !candles) {
      return
    }

    const cutoffMs = Date.now() - windowSeconds * 1000
    const lineData = normalizeLinePoints(history[selectedAsset], cutoffMs)
    const historySignature: HistorySignature = {
      key: `${selectedAsset}:${chartStyle}:${windowSeconds}`,
      first: lineData.length > 0 ? lineData[0].time : null,
      last: lineData.length > 0 ? lineData[lineData.length - 1].time : null,
      len: lineData.length,
    }
    const previousSignature = historySignatureRef.current
    const isMonotonicAppend =
      previousSignature !== null
      && previousSignature.key === historySignature.key
      && previousSignature.last !== null
      && historySignature.last !== null
      && historySignature.last >= previousSignature.last
      && historySignature.len >= previousSignature.len
      && (
        previousSignature.first === null
        || historySignature.first === null
        || historySignature.first >= previousSignature.first
      )

    // Skip full series replacement on steady stream appends.
    if (isMonotonicAppend) {
      historySignatureRef.current = historySignature
      return
    }

    const bucketSecs =
      chartStyle === 'polymarket'
        ? POLYMARKET_CANDLE_BUCKET_SECS
        : resolveCandleBucketSecs(lineData)
    candleBucketSecsRef.current = bucketSecs
    const candleData = buildCandles(lineData, bucketSecs)

    area.setData(lineData)
    candles.setData(candleData)
    lastLineTimeRef.current = lineData.length > 0 ? lineData[lineData.length - 1].time : null
    lastCandleRef.current = candleData.length > 0 ? candleData[candleData.length - 1] : null
    lastVisualUpdateMsRef.current = 0

    if (lineData.length > 0) {
      if (chartStyle === 'polymarket') {
        const barCount = Math.max(1, candleData.length)
        const visibleBars = Math.min(POLYMARKET_MAX_VISIBLE_BARS, Math.max(6, barCount + 2))
        const to = barCount - 1 + 0.6
        const from = Math.max(-0.5, to - visibleBars)
        chart.timeScale().setVisibleLogicalRange({ from, to })
      } else {
        chart.timeScale().fitContent()
      }
      chart.timeScale().scrollToRealTime()
    }
    historySignatureRef.current = historySignature
  }, [chartStyle, history, selectedAsset, windowSeconds])

  useEffect(() => {
    const chart = chartRef.current
    const area = areaSeriesRef.current
    const candles = candleSeriesRef.current
    if (!chart || !area || !candles) {
      return
    }

    const livePrice = livePrices[selectedAsset]
    if (!livePrice) {
      return
    }

    const visualStepMs = chartStyle === 'polymarket' ? POLYMARKET_VISUAL_STEP_MS : CYBER_VISUAL_STEP_MS
    const liveTimestampMs = normalizeTimestampMs(livePrice.timestamp)
    const quantizedMs = Math.floor(liveTimestampMs / visualStepMs) * visualStepMs
    if (quantizedMs <= lastVisualUpdateMsRef.current) {
      return
    }
    lastVisualUpdateMsRef.current = quantizedMs

    const nextTime = toChartTime(quantizedMs)
    const lastLineTime = lastLineTimeRef.current
    if (lastLineTime !== null && nextTime < lastLineTime) {
      return
    }

    const value = livePrice.price
    area.update({ time: nextTime, value })
    lastLineTimeRef.current = nextTime

    const bucketSecs =
      chartStyle === 'polymarket' ? POLYMARKET_CANDLE_BUCKET_SECS : candleBucketSecsRef.current
    const bucketTime = (Math.floor(Number(nextTime) / bucketSecs) * bucketSecs) as UTCTimestamp
    const lastCandle = lastCandleRef.current
    if (lastCandle && bucketTime < lastCandle.time) {
      return
    }

    if (!lastCandle || bucketTime > lastCandle.time) {
      const seed: CandlePoint = {
        time: bucketTime,
        open: value,
        high: value,
        low: value,
        close: value,
      }
      candles.update(seed)
      lastCandleRef.current = seed
      chart.timeScale().scrollToRealTime()
      return
    }

    const merged: CandlePoint = {
      time: lastCandle.time,
      open: lastCandle.open,
      high: Math.max(lastCandle.high, value),
      low: Math.min(lastCandle.low, value),
      close: value,
    }
    candles.update(merged)
    lastCandleRef.current = merged
    chart.timeScale().scrollToRealTime()
  }, [chartStyle, livePrices, selectedAsset])

  return <div className="price-chart" ref={containerRef} />
}
