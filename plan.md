Plan: Rust Directional Trading Bot for Polymarket (15m/1h BTC/ETH/SOL/XRP)
TL;DR: Build a modular, event-driven Rust bot that predicts UP/DOWN direction on Polymarket 15m/1h markets using multi-CEX price analysis (Binance, Coinbase, Bybit). The bot uses technical indicators + order book microstructure + momentum analysis to generate confidence-weighted predictions. Starts with rules-based logic, designed to add ML later. All configurable, with CSV persistence and structured logging.

Steps
Phase 1: Project Foundation & Core Infrastructure
Initialize Rust project structure at Cargo.toml with workspace layout:

Binary crate: polybot (main application)
Library crates: oracle, strategy, execution, persistence
Set up core dependencies in Cargo.toml:

tokio (async runtime with full features)
tokio-tungstenite (WebSocket client)
reqwest (HTTP client with rustls)
serde + serde_json (JSON)
ethers or alloy (EIP-712 signing for Polymarket)
tracing + tracing-subscriber (structured logging)
chrono (timestamps)
csv (trade persistence)
anyhow + thiserror (error handling)
Create configuration system at src/config/mod.rs:

YAML config files per experiment (no secrets)
.env file for API keys, private keys, endpoints
Validation on startup with "config digest" (sanitized)
Implement structured logging at src/logging.rs:

JSON format with BOT_TAG, market, asset, timestamp, latency_ms
File rotation + console output
Latency tracking per stage (ingest → feature → decision → order → fill)
Phase 2: CEX Oracle (Multi-Source Price Aggregation)
Create WebSocket connection manager at src/oracle/connection.rs:

Auto-reconnect with exponential backoff
Connection health monitoring
Graceful shutdown handling
Implement Binance WebSocket client at src/oracle/sources/binance.rs:

Connect to wss://stream.binance.com:9443/ws/{symbol}@aggTrade
Connect to wss://stream.binance.com:9443/ws/{symbol}@bookTicker
Normalize to Trade and Quote structs with timestamps
Implement Bybit WebSocket client at src/oracle/sources/bybit.rs:

Connect to wss://stream.bybit.com/v5/public/spot
Subscribe to publicTrade.{symbol} and orderbook.50.{symbol}
Handle heartbeat ({"op": "ping"} every 20s)
Implement Coinbase WebSocket client at src/oracle/sources/coinbase.rs:

Connect to wss://advanced-trade-ws.coinbase.com
Subscribe to market_trades and ticker_batch channels
Handle JWT auth for future private channels
Build price aggregator at src/oracle/aggregator.rs:

Normalize all sources to NormalizedTick { ts, bid, ask, mid, source, latency_ms }
Compute weighted mid-price (weight by source reliability/latency)
Calculate confidence score based on source agreement
Expose via mpsc::channel for downstream consumers
Create candle builder at src/oracle/candles.rs:

Build 10s, 1m, 15m, 1h candles from trade stream
Store recent candles in ring buffer (last 100 of each timeframe)
Support historical fetch via Binance REST API for backtesting
Phase 3: Feature Engine (Technical Analysis)
Implement TA indicators at src/features/indicators.rs:

RSI (14-period on 1m candles)
MACD (12, 26, 9 on 5m candles)
VWAP (session-based)
Bollinger Bands (20-period)
ATR (14-period for volatility)
Heikin Ashi conversion
Implement microstructure features at src/features/microstructure.rs:

Order book imbalance (bid/ask volume ratio)
Spread analysis (bps)
Trade intensity (trades per second)
Volume-weighted price momentum
Implement momentum features at src/features/momentum.rs:

Price velocity (% change per minute)
Acceleration (velocity derivative)
Cross-exchange momentum divergence
Build feature aggregator at src/features/aggregator.rs:

Combine all features into FeatureSet struct
Compute feature timestamps and staleness detection
Export feature vector for ML (future use)
Phase 4: Strategy Engine (Directional Prediction)
Design prediction model interface at src/strategy/model.rs:

Prediction { direction: UP/DOWN, confidence: f64 (0.5-1.0), features: FeatureSet }
Trait Predictor with predict(features) -> Prediction
Implement rules-based predictor at src/strategy/rules.rs:

Confluence scoring: RSI + MACD + momentum + book imbalance
Minimum confidence threshold (configurable, default 0.60)
Signal strength calculation
Implement signal filter at src/strategy/filters.rs:

Minimum spread requirement
Volatility regime detection (pause in high vol)
Time-to-expiry minimum (no trades in last 30 seconds of market)
Oracle confidence minimum
Build strategy orchestrator at src/strategy/orchestrator.rs:

OODA loop: Observe (features) → Orient (context) → Decide (prediction) → Act (signal)
Track market state per asset/timeframe
Rolling analysis with configurable entry delay
Phase 5: Risk Management
Implement risk manager at src/risk/manager.rs:

MAX_POSITION_SIZE_USDC per trade
MAX_DAILY_LOSS_USDC (kill switch trigger)
MAX_OPEN_POSITIONS per market
MAX_DRAWDOWN_PCT (circuit breaker)
Confidence-weighted position sizing (60% conf = 50% size, 80%+ = 100% size)
Implement circuit breakers at src/risk/circuit_breakers.rs:

Oracle divergence detector (CEX vs Polymarket mid)
WS health monitor (reconnect frequency)
Fee/spike detector
Kill switch activation logic
Phase 6: Polymarket Execution
Implement CLOB client at src/execution/clob.rs:

REST client for https://clob.polymarket.com
EIP-712 order signing (use ethers crate)
Order structure: maker, signer, tokenId, makerAmount, takerAmount, expiration, nonce, feeRateBps, signatureType
Implement market discovery at src/execution/discovery.rs:

Find active 15m/1h markets for BTC/ETH/SOL/XRP
Map: market_slug → condition_id → token_ids(UP/DOWN) → end_time → tick_size
Cache with periodic refresh
Implement order executor at src/execution/orders.rs:

Place marketable limit orders (for quick fills)
Handle partial fills
Cancel/replace logic
Order status tracking
Connect to Polymarket WebSocket at src/execution/websocket.rs:

Market channel: book updates, trades
User channel: fills, order status
Position tracking
Phase 7: Persistence & Auditing
Implement CSV trade logger at src/persistence/trades.rs:

Path: data/trades/{BOT_TAG}/trades_YYYY-MM-DD.csv
Schema: ts_open, ts_close, asset, timeframe, market_slug, strategy_id, side, entry_px, exit_px, size, fee_paid, result, pnl_usdc, pnl_bps, latency_submit_ms, latency_fill_ms, oracle_mid_open, polymarket_mid_open, confidence, notes_json
Implement daily summary at src/persistence/summary.rs:

Path: data/summary/{BOT_TAG}/summary_YYYY-MM-DD.csv
Metrics: total_trades, wins, losses, win_rate, total_pnl, avg_latency, max_drawdown
Implement feature/price recorder at src/persistence/recorder.rs:

Optional recording of all features and oracle prices
For backtesting and ML training data
Phase 8: Integration & Main Loop
Build main application at src/main.rs:

Initialize all components
Spawn tokio tasks for each WebSocket connection
Central event loop consuming from channels
Graceful shutdown handler
Implement supervisor at src/supervisor.rs:

Health checks for all components
Automatic restart on failure
Metrics exposition (Prometheus format optional)
Phase 9: Backtesting Framework
Implement backtest engine at src/backtest/engine.rs:

Load historical data from CEX APIs (Binance klines)
Replay through feature engine
Simulate trades with realistic slippage/fees
Calculate metrics: win rate, Sharpe, max DD, profit factor
Implement forward test mode at src/backtest/paper.rs:

Live data, paper trades only
Compare predicted vs actual outcomes
Validate model before going live
Phase 10: Documentation & Deployment
Create configuration files:

config/default.yaml - Default parameters
config/prod.yaml - Production overrides
.env.example - Template for secrets
Create deployment files:

Dockerfile - Multi-stage Rust build
docker-compose.yaml - Service definition
deploy/systemd.service - Systemd unit file
Write documentation:

README.md - Setup, configuration, usage
docs/ARCHITECTURE.md - System design
docs/STRATEGY.md - Strategy logic explanation
Verification
Unit tests: Each module has tests in src/**/tests.rs
Integration tests: tests/integration/ with mock WebSocket servers
Backtest validation: Run against 30+ days of historical data, verify edge exists
Forward test: Run in paper mode for 1-2 weeks before live
Health checks: All WebSocket connections maintain 5s reconnect time
Latency benchmarks: Feature computation 10ms, order submission 500ms
Decisions
Decision	Choice	Rationale
Stack	Rust	User preference for low latency/robustness, even with slower iteration
Strategy	Directional (UP/DOWN)	User believes arbitrage won't work on Polymarket; wants trend prediction
Prediction output	Binary + Confidence	Enables confidence-weighted position sizing, trades only when edge is clear
Analysis components	TA + Microstructure + Momentum + CEX Correlations	Comprehensive feature set for robust predictions
Entry timing	Delayed + Rolling	Wait to gather data, analyze throughout period
Position sizing	Confidence-weighted	60% conf = 50% size, 80%+ = 100% size; filters for quality
ML approach	Hybrid (rules → ML)	Start interpretable, add ML once data is collected
CEX sources	Binance, Coinbase, Bybit	User selection; good liquidity/coverage
Chainlink	No	CEX-only oracle for simplicity, no additional costs
Currency	USDC only	Polymarket's native currency, keep it simple
Backtest data	CEX historical APIs	Use Binance/Coinbase klines for initial backtesting
UI	Logging only (MVP)	Structured logs first, TUI later if needed
Polymarket SDK	None (use REST directly)	No Rust SDK exists; implement EIP-712 signing with ethers crate
Milestones
Milestone	Description	Estimated Effort
M0	Project setup, config, logging	1-2 days
M1	CEX Oracle (Binance only)	2-3 days
M2	Multi-CEX Oracle (add Coinbase, Bybit)	2-3 days
M3	Feature Engine (TA + microstructure)	3-4 days
M4	Strategy Engine (rules-based predictor)	3-4 days
M5	Risk Manager + Circuit Breakers	2 days
M6	Polymarket Execution (CLOB + WS)	4-5 days
M7	Persistence (CSV) + Integration	2 days
M8	Backtest Framework	3 days
M9	Forward Test (paper mode)	1 week observation
M10	Documentation + Deployment	2 days
Total MVP (M0-M7): ~3-4 weeks
Full system with backtesting (M0-M10): ~5-6 weeks

