import { useState, useEffect, useRef } from "react";
import type { DashboardState, WsMessage } from "../types/dashboard";

const WS_URL = "ws://localhost:3000/ws";
const PRICE_UPDATE_INTERVAL_MS = 2000; // Only update prices every 2 seconds

export function useWebSocket() {
  const [state, setState] = useState<DashboardState | null>(null);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimeoutRef = useRef<number | null>(null);
  const mountedRef = useRef(false);
  const lastPriceUpdateRef = useRef<number>(0);
  const pendingPricesRef = useRef<Record<string, import("../types/dashboard").AssetPrice> | null>(null);

  useEffect(() => {
    mountedRef.current = true;

    const connect = () => {
      if (!mountedRef.current) return;
      if (wsRef.current?.readyState === WebSocket.OPEN) return;

      const ws = new WebSocket(WS_URL);
      wsRef.current = ws;

      ws.onopen = () => {
        if (!mountedRef.current) return;
        setConnected(true);
        setError(null);
        console.log("🔌 WebSocket connected");
      };

      ws.onmessage = (event) => {
        if (!mountedRef.current) return;
        try {
          const message: WsMessage = JSON.parse(event.data);
          
          if (message.type === "FullState") {
            setState(message.data);
          } else if (message.type === "StatsUpdate") {
            setState((prev) =>
              prev
                ? {
                    ...prev,
                    paper: { ...prev.paper, stats: message.data },
                  }
                : null
            );
          } else if (message.type === "PriceUpdate") {
            // Throttle price updates to prevent UI flickering
            const now = Date.now();
            const timeSinceLastUpdate = now - lastPriceUpdateRef.current;
            
            if (timeSinceLastUpdate >= PRICE_UPDATE_INTERVAL_MS) {
              // Enough time has passed, update immediately
              lastPriceUpdateRef.current = now;
              setState((prev) =>
                prev
                  ? {
                      ...prev,
                      prices: {
                        prices: message.data,
                        last_update: now,
                      },
                    }
                  : null
              );
            } else {
              // Store pending prices and set a timer to apply them
              pendingPricesRef.current = message.data;
            }
          } else if (message.type === "NewTrade") {
            setState((prev) =>
              prev
                ? {
                    ...prev,
                    paper: {
                      ...prev.paper,
                      recent_trades: [message.data, ...prev.paper.recent_trades].slice(0, 50),
                    },
                  }
                : null
            );
          } else if (message.type === "NewSignal") {
            // Could store signals if needed
          } else if (message.type === "PositionOpened") {
            setState((prev) =>
              prev
                ? {
                    ...prev,
                    paper: {
                      ...prev.paper,
                      open_positions: [...prev.paper.open_positions, message.data],
                    },
                  }
                : null
            );
          } else if (message.type === "PositionClosed") {
            setState((prev) =>
              prev
                ? {
                    ...prev,
                    paper: {
                      ...prev.paper,
                      open_positions: prev.paper.open_positions.filter(
                        (p) => p.id !== message.data.position_id
                      ),
                      recent_trades: [message.data.trade, ...prev.paper.recent_trades].slice(0, 50),
                    },
                  }
                : null
            );
          }
        } catch (e) {
          console.error("Failed to parse WebSocket message:", e);
        }
      };

      ws.onclose = () => {
        if (!mountedRef.current) return;
        setConnected(false);
        console.log("🔌 WebSocket disconnected");
        
        // Reconnect after 3 seconds
        reconnectTimeoutRef.current = window.setTimeout(() => {
          connect();
        }, 3000);
      };

      ws.onerror = () => {
        if (!mountedRef.current) return;
        setError("WebSocket connection error");
        setConnected(false);
      };
    };

    connect();

    // Timer to apply pending price updates
    const priceUpdateTimer = setInterval(() => {
      if (pendingPricesRef.current) {
        const now = Date.now();
        if (now - lastPriceUpdateRef.current >= PRICE_UPDATE_INTERVAL_MS) {
          lastPriceUpdateRef.current = now;
          setState((prev) =>
            prev
              ? {
                  ...prev,
                  prices: {
                    prices: pendingPricesRef.current!,
                    last_update: now,
                  },
                }
              : null
          );
          pendingPricesRef.current = null;
        }
      }
    }, PRICE_UPDATE_INTERVAL_MS);

    return () => {
      mountedRef.current = false;
      clearInterval(priceUpdateTimer);
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current);
      }
      wsRef.current?.close();
    };
  }, []);

  return { state, connected, error };
}
