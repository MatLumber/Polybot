# PolyBot

Directional trading bot for Polymarket 15m/1h BTC/ETH/SOL/XRP markets using native RTDS + CLOB market data.

## Features

- **Directional Trading**: Predicts UP/DOWN direction for crypto markets
- **Adaptive Calibration**: Learns from past trades to weight indicators by market
- **Multiple Data Sources**: Polymarket RTDS (native), CLOB orderbook, optional Binance
- **Paper Trading**: Safe testing mode with real market prices
- **Risk Management**: Trailing stops, take profit, daily loss limits, Kelly sizing
- **Real-time Dashboard**: WebSocket-based monitoring interface

## Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   RTDS Feed     │────▶│  Feature Engine │────▶│ Strategy Engine │
│  (Polymarket)   │     │   (Indicators)  │     │  (Signals)      │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                                        │
┌─────────────────┐     ┌─────────────────┐            ▼
│  Orderbook Feed │────▶│  Paper Trading  │◀──────▶ Risk Manager
│   (WebSocket)   │     │     Engine      │            │
└─────────────────┘     └─────────────────┘            ▼
                              │                  ┌─────────────┐
                              ▼                  │ CLOB Client │
                        ┌─────────────┐          │ (Execution) │
                        │  Dashboard  │          └─────────────┘
                        │   (API/WS)  │
                        └─────────────┘
```

## Quick Start

### Prerequisites

- Rust 1.75+
- Polygon wallet with USDC

### Installation

```bash
# Clone
git clone https://github.com/YOUR_USERNAME/polybot.git
cd polybot

# Build
cargo build --release --features dashboard

# Configure
cp .env.example .env
# Edit .env with your credentials
```

### Configuration

```env
# Required
PRIVATE_KEY=0xYOUR_PRIVATE_KEY
POLYMARKET_ADDRESS=0xYOUR_ADDRESS

# Safety (start with dry_run=true)
POLYBOT__BOT__DRY_RUN=true

# Dashboard port
DASHBOARD_PORT=8088
```

### Run

```bash
# Paper trading (safe)
./target/release/polybot

# With dashboard
./target/release/polybot
# Dashboard available at http://localhost:8088
```

## API Endpoints

### Trading Data

| Endpoint | Description |
|----------|-------------|
| `GET /api/stats` | Complete dashboard state |
| `GET /api/trades` | Recent trades |
| `GET /api/positions` | Open positions |
| `GET /api/prices` | Current prices |
| `GET /api/health` | Feed health status |

### Calibration

| Endpoint | Description |
|----------|-------------|
| `GET /api/calibration/markets` | Learning progress by market |
| `GET /api/calibration/quality` | ECE/Brier calibration metrics |
| `GET /api/indicator-stats` | Per-indicator statistics |

### Data Export

| Endpoint | Description |
|----------|-------------|
| `GET /api/data/calibrator` | Brain state (JSON) |
| `GET /api/data/paper-state` | Paper trading state (JSON) |
| `GET /api/data/trades` | Download trades CSV |
| `GET /api/data/signals` | Download signals CSV |
| `GET /api/data/prices` | Download prices CSV |
| `GET /api/data/rejections` | Download rejections CSV |
| `GET /api/data/files` | List all data files |

### WebSocket

```
ws://localhost:8088/ws
```

Real-time updates for prices, trades, positions, and signals.

## Strategy

The bot uses a weighted combination of technical indicators:

| Indicator | Default Weight | Description |
|-----------|----------------|-------------|
| RSI Extreme | 1.5 | Overbought/oversold conditions |
| MACD Histogram | 1.5 | Momentum direction |
| EMA Trend | 1.5 | Trend alignment |
| Bollinger Band | 2.0 | Volatility breakout |
| ADX Trend | 2.5 | Trend strength |
| Momentum Acceleration | 2.0 | Price velocity |
| Heikin Ashi | 1.0 | Smoothed candle direction |
| Stoch RSI | 1.0 | Stochastic oscillator |

Weights are **automatically calibrated** based on historical win rates per market.

## Calibration Process

1. Bot starts with default weights
2. Each trade records which indicators triggered the signal
3. On trade exit, win/loss updates indicator statistics
4. Weights adjust based on empirical win rates
5. Markets need ~30 trades for reliable calibration

## Risk Management

- **Position Sizing**: Kelly criterion (optional)
- **Trailing Stop**: Lock in profits on pullback
- **Take Profit**: Auto-exit at target ROI
- **Hard Stop**: Maximum loss per trade
- **Daily Loss Limit**: Stop trading after threshold
- **Time Stop**: Exit before market expiry

## Deployment

### Systemd Service (Linux)

```bash
sudo tee /etc/systemd/system/polybot.service > /dev/null << 'EOF'
[Unit]
Description=PolyBot Trading Bot
After=network.target

[Service]
Type=simple
User=root
WorkingDirectory=/root/polybot
Environment="PATH=/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
ExecStart=/root/polybot/target/release/polybot
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable polybot
sudo systemctl start polybot
```

### Logs

```bash
# View logs
sudo journalctl -u polybot -f

# Check status
sudo systemctl status polybot
```

## Project Structure

```
polybot/
├── src/
│   ├── main.rs              # Entry point
│   ├── paper_trading.rs     # Paper trading engine
│   ├── strategy/            # Signal generation & calibration
│   ├── features/            # Technical indicators
│   ├── oracle/              # Price aggregation & sources
│   ├── polymarket/          # CLOB client & execution
│   ├── clob/                # Orderbook feed
│   ├── risk/                # Risk management
│   ├── persistence/         # CSV storage
│   ├── dashboard/           # HTTP/WebSocket API
│   └── config/              # Configuration
├── dashboard/               # React frontend (optional)
├── data/                    # Persistent storage
│   ├── calibrator_state_v2.json
│   ├── paper_trading_state.json
│   ├── trades/
│   ├── signals/
│   └── prices/
└── config/
    └── default.yaml
```

## Safety Warnings

- **NEVER** commit `.env` to version control
- **ALWAYS** start with `POLYBOT__BOT__DRY_RUN=true`
- Paper trading uses real market prices but fake execution
- The bot can lose money in live mode

## License

MIT

## Disclaimer

This software is for educational purposes. Trading involves risk of loss. Past performance does not guarantee future results. Use at your own risk.
