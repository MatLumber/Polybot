# 🤖 PolyBot ML v3.0

Sistema de Trading con Machine Learning para Polymarket

## 🎯 Características ML

- **Ensemble de 3 Modelos**: Random Forest (40%) + Gradient Boosting (35%) + Logistic Regression (25%)
- **50 Features**: Técnicos, microestructura, temporales, y cross-asset
- **Smart Filters**: Liquidez, spread, volatilidad, horarios óptimos
- **Entrenamiento Continuo**: Walk-forward validation + ajuste dinámico de pesos
- **Calibración**: Isotonic regression para probabilidades calibradas
- **Target**: 55-60% win rate

## 🚀 Inicio Rápido

### 1. Activar ML Strategy

Edita `config/local.yaml` (o copia `config/v3.yaml.example`):

```yaml
# Activar V3 ML
use_v3_strategy: true

# Configuración ML
ml_engine:
  enabled: true
  model_type: "ensemble"
  min_confidence: 0.55
  
  # Entrenamiento
  retrain_interval_trades: 50
  min_samples_for_training: 30
  
  # Pesos ensemble (deben sumar 1.0)
  random_forest_weight: 0.40
  gradient_boosting_weight: 0.35
  logistic_regression_weight: 0.25
  dynamic_weight_adjustment: true
  
  # Filtros
  max_spread_bps_15m: 100
  max_spread_bps_1h: 150
  min_depth_usdc: 5000
  max_volatility_5m: 0.02
```

### 2. Iniciar

```bash
# Linux/Mac
./start.sh

# O manualmente
cargo run --release
```

### 3. Ver Dashboard

Abre http://localhost:3000

## 📊 Dashboard ML - Nuevo!

### Endpoints REST

- `GET /api/ml/state` - Estado del ML Engine
- `GET /api/ml/metrics` - Métricas de performance
- `GET /api/ml/models` - Información de modelos
- `GET /api/ml/features` - Importancia de features
- `GET /api/ml/training` - Estado de entrenamiento

### WebSocket (Tiempo Real)

Conecta a `ws://localhost:3000/ws` para recibir:

```json
// ML State Update
{
  "type": "MLStateUpdate",
  "data": {
    "enabled": true,
    "model_type": "Ensemble",
    "version": "3.0",
    "timestamp": 1234567890
  }
}

// ML Prediction
{
  "type": "MLPrediction",
  "data": {
    "asset": "BTC",
    "timeframe": "15m",
    "direction": "Up",
    "confidence": 0.72,
    "prob_up": 0.68,
    "model_name": "Ensemble",
    "features_triggered": ["RSI_oversold", "MACD_bullish"],
    "timestamp": 1234567890
  }
}

// ML Metrics
{
  "type": "MLMetricsUpdate",
  "data": {
    "accuracy": 0.58,
    "win_rate": 0.56,
    "total_predictions": 150,
    "correct_predictions": 84,
    "ensemble_weights": [
      {"name": "Random Forest", "weight": 0.42, "accuracy": 0.60},
      {"name": "Gradient Boosting", "weight": 0.33, "accuracy": 0.57},
      {"name": "Logistic Regression", "weight": 0.25, "accuracy": 0.55}
    ],
    "timestamp": 1234567890
  }
}
```

## 📈 Features (50 total)

### Técnicos (14)
- RSI + normalizado + divergencia
- MACD + señal + histograma + pendiente
- Bollinger Bands posición + ancho + squeeze
- ADX + DI+ + DI- + fuerza de tendencia

### Momentum (6)
- Velocidad de precio
- Aceleración
- Momentum 2do orden
- VWAP distance
- StochRSI + señales

### Microestructura (7)
- Spread (bps + percentil)
- Orderbook imbalance
- Depth top 5
- Concentración de liquidez
- Intensidad de trades
- Order flow imbalance

### Temporales (9)
- Minutos hasta cierre
- Progreso de ventana
- Hora del día (encoding cíclico)
- Día de semana
- Es fin de semana
- Minutos desde apertura

### Contexto (10)
- Régimen de mercado
- Volatilidad + percentil
- Correlación BTC-ETH
- Cambio de correlación
- Sentimiento de mercado

### Calibrador (4)
- Confianza del calibrador
- Indicadores de acuerdo
- Win rate promedio
- Pesos bullish/bearish

## 🔧 Configuración Avanzada

### Cambiar Pesos del Ensemble

```yaml
ml_engine:
  random_forest_weight: 0.50      # Más conservador
  gradient_boosting_weight: 0.30
  logistic_regression_weight: 0.20
```

### Ajustar Filtros

```yaml
ml_engine:
  max_spread_bps_15m: 80          # Más estricto
  max_spread_bps_1h: 120
  min_depth_usdc: 10000           # Más liquidez requerida
  max_volatility_5m: 0.015        # Menos volatilidad
  optimal_hours_only: true        # Solo horarios óptimos
```

### Entrenamiento

```yaml
ml_engine:
  retrain_interval_trades: 30     # Re-entrenar cada 30 trades
  min_samples_for_training: 20    # Mínimo para entrenar
```

## 🧪 Tests

```bash
# Todos los tests
cargo test

# Solo ML
cargo test ml_engine

# Tests específicos
cargo test test_random_forest_training
cargo test test_logistic_regression_training
cargo test test_ensemble_predictor
```

## 📁 Estructura ML

```
src/ml_engine/
├── mod.rs              # Configuración y estado
├── models/
│   └── mod.rs          # Ensemble (RF, GB, LR)
├── features.rs         # 50 features
├── filters.rs          # Smart filters
├── calibration.rs      # Calibración de probabilidades
├── dataset.rs          # Dataset management
├── training.rs         # Walk-forward validation
├── predictor.rs        # ML predictor
├── data_client.rs      # Polymarket data downloader
└── integration.rs      # V2/V3 integration

src/strategy/
├── v3_strategy.rs      # ML Strategy
└── ...
```

## 🎓 Aprendizaje

El bot aprende automáticamente:

1. **Features**: Calcula 50 features cada tick
2. **Predicción**: Ensemble de 3 modelos predice dirección
3. **Filtros**: Smart filters validan señal
4. **Trade**: Si pasa filtros, ejecuta trade
5. **Feedback**: Al cerrar, registra resultado
6. **Ajuste**: Actualiza pesos dinámicamente cada 10 trades
7. **Retraining**: Re-entrena cada 50 trades con datos nuevos

## 📊 Métricas Clave

- **Accuracy**: Predicciones correctas / total
- **Win Rate**: Trades ganados / total trades
- **Confidence**: Confianza del ensemble (0-1)
- **Edge**: Ventaja esperada por trade
- **Calibration**: ECE (Expected Calibration Error)

## 🔒 Seguridad

- Paper trading habilitado por defecto
- Kill switch disponible
- Límites de riesgo configurables
- Todos los cambios se guardan en disco

## 🆘 Soporte

Si tienes problemas:

1. Verifica logs: `RUST_LOG=info cargo run`
2. Revisa tests: `cargo test`
3. Consulta dashboard: http://localhost:3000
4. Revisa configuración: `config/local.yaml`

## 📝 Changelog v3.0

- ✅ Ensemble de 3 modelos con SmartCore
- ✅ 50 features calculadas en tiempo real
- ✅ Smart filters adaptativos
- ✅ Dynamic weight adjustment
- ✅ Walk-forward validation
- ✅ Calibración de probabilidades
- ✅ Dashboard con endpoints ML
- ✅ WebSocket tiempo real
- ✅ 21 tests pasando

---

**Listo para operar con ML inteligente!** 🚀📈
