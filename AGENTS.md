# PolyBot - AGENTS.md (Comprehensive Guide for AI Agents)

Este documento está diseñado estrictamente para proveer contexto inmediato, detallado y absoluto a cualquier agente de Inteligencia Artificial ("AI Agent") u operador DevOps que vaya a trabajar en **PolyBot**. Sirve como la principal fuente de verdad arquitectónica, funcional y conceptual.

---

## 1. Visión General del Proyecto (Overview)
- **¿Qué es?** PolyBot es un bot de trading de alta frecuencia y direccional (no arbitraje puro) especializado en el ecosistema cripto usando **Polymarket**.
- **Mercados Objetivo:** Especializado en mercados temporales de criptomonedas (Ej: Bitcoin/Ethereum/Solana a 15 min, 1 hora).
- **Entradas (Data Sources):** Extrae datos usando RTDS (Real-Time Data Streams nativos de Polymarket a través de WebSockets) y el CLOB (Central Limit Order Book).
- **Procesamiento:** Usa un motor de Estrategia acoplado a un motor de Machine Learning (`ml_engine`) que provee calibraciones adaptativas (Brier Score/ECE) para ponderar dinámicamente varios indicadores de análisis técnico (Features).
- **Salidas (Execution):** Opera mediante un módulo de Paper Trading (por defecto) o trading real (Live) interaccionando con los contratos inteligentes descentralizados mediante firmas EIP-712.
- **Herramientas del Sistema:** Posee métricas de monitoreo continuo (HTTP/WS API) comunicadas con un Dashboard (Frontend web), y un subsistema rígido de Risk Management (Kelly Sizing, Daily Loss, Trailing Stops).

---

## 2. Tecnologías y Stack

### Backend y Lógica Core (Rust)
- **Lenguaje:** Rust (1.75+)
- **Runtime Asíncrono:** Tokio (`tokio-tungstenite` para WebSockets, `reqwest` para cliente HTTP).
- **API Web / Dashboard Backend:** Axum (gestiona los endpoints y el tunnel WebSocket para enviar la data en tiempo real al dashboard).
- **Criptografía y Web3:** `ethers-rs` (para firmas EIP-712 interactuando con Polygon), encriptado TLS nativo (`native-tls`).
- **Data Science / ML:** `smartcore` y librerías de `ndarray` para vectorizaciones. Cálculos numéricos precisos usando `rust_decimal`.
- **Persistencia:** CSV, JSON (vía `serde` y `bincode`).

### Frontend (Dashboard)
- **Lenguaje:** TypeScript, React (configurado usando Vite).
- **Diseño:** TailwindCSS, clases de utilidad rápidas.
- **Estado:** Se comunica vía WS con el backend (puerto usual 8088 por defecto) para pintar órdenes, la salud de las APIs y gráficos del bot.

---

## 3. Entendiendo Polymarket (Trading No Convencional)

Polymarket **NO** es un exchange convencional como Binance o Coinbase. Es vital que el agente comprenda estas diferencias antes de codificar lógicas de ordenes:
1. **Mercados de Predicción (Prediction Markets):** Se apuesta a la probabilidad de un evento en el mundo real en forma de contratos (Tokens Condicionales). Ej: "¿El precio de BTC será mayor a $95k al acabar la hora?".
2. **Resultados Binarios (Outcomes):** Estos mercados suelen resolverse en `YES` o `NO`.
3. **Probabilidad == Precio:** El precio de un token YES u NO se cotiza de `$0.00` a `$1.00`. Ese precio refleja directamente el porcentaje de probabilidad del mercado. Por ejemplo, comprar a `$0.40` significa que el mercado estima la posibilidad en un 40%.
4. **Acoplamiento CTF (Conditional Token Framework):**
    - Todo mercado líquido respeta esto: un resultado de $1.00 se reparte. Las probabilidades totales deben sumar 100%. Así que $0.40(YES) + $0.60(NO) = $1.00 (USDC).
    - **Merge:** Teniendo cantidades equivalentes de tokens `YES` y `NO`, pueden fusionarse para rescatar USDC.e del CTO sin tocar el orderbook.
    - **Split:** USDC.e puede dividirse creando liquidez (1 YES + 1 NO).
5. **Central Limit Order Book (CLOB):** El ruteo de órdenes límite es gestionado por servidores centralizados (gamma-api/clob). Las confirmaciones on-chain operan al hacer matching off-chain.
6. **Autenticación (Sin Gas Fees):** El bot crea y cancela órdenes firmando estructuras de datos JSON basadas en EIP-712. Nunca interactúa directamente enviando gas al colocar órdenes CLOB; en vez de eso, envía payloads pre-firmados por una billetera de la red Polygon que tenga fondos habilitados (USDC.e proxy).
7. **Dinámica PolyBot y Mercados 15m/1h:** Los mercados de **15 minutos y 1 hora** en criptomonedas (Ej: BTC/ETH a 15m) funcionan como una predicción binaria sobre si el precio cerrará **arriba (UP)** o **abajo (DOWN)** de un precio _strike_ específico al finalizar el periodo.
    - PolyBot lee el Orderbook del mercado 15m/1h, evalúa la información con sus indicadores (features) y el ML.
    - Si predice que el precio subirá respecto al strike, **compra YES**.
    - Si predice que el precio bajará, **compra NO**.
    - Al acercarse al tiempo objetivo (expiry), o si se activa un Stop Loss/Take Profit, el bot manda órdenes CLOB límite para salir de la posición antes de la resolución del mercado.

---

## 4. Estructura del Proyecto (Worktree Details)

A continuación un mapa conceptual del árbol de archivos y su propósito nativo:

```text
polybot/
├── src/                     # Código fuente del Backend Rust
│   ├── main.rs              # Punto de entrada. Inicializa el bot, carga configuraciones y arranca los hilos (threads).
│   ├── lib.rs               # Define los módulos y su visibilidad interna pública a lo largo de la crate.
│   ├── config/              # Lectura de configuraciones desde `config/*.yaml` combinados con variables `.env`.
│   ├── clob/                # Clientes especializados para el backend Polymarket CLOB. Websockets de profundidad de mercado y API REST.
│   ├── oracle/              # Orquestador multi-fuente de datos. Recopila RTDS (Polymarket), Binance/Pyth si está configurado.
│   ├── polymarket/          # Definición de tokens, utilidades para interaccionar con el Protocolo, Auth EIP-712.
│   ├── features/            # Indicadores técnicos en crudo (Ej: MACD, RSI Extreme, Bollinger Bands, Heikin Ashi). Generan valores a cada tick.
│   ├── strategy/            # Recolecta un array de features, toma decisiones sobre pesos probatorios y emite "Signals".
│   ├── ml_engine/           # Lógica autónoma predictiva. Calibrador adaptativo. Modifica los pesos de los features si el historial demuestra que x indicador acierta más en determinado mercado.
│   ├── risk/                # Módulo crítico. Liquidador estricto de Stop Loss, Take Profit, trailing stops fijos y reglas Kelly de tamaño de posición.
│   ├── paper_trading.rs     # Sandbox Engine. Se comporta como Polymarket internamente pero en simulación para pruebas seguras 100%.
│   ├── persistence/         # Handlers que guardan historial de trades/señales en data/.
│   ├── dashboard/           # API Axum. Monta rutas REST (/api/stats) y el Router WebSocket para UI.
│   ├── backtesting/         # Funcionalidad y rutinas paramétricas que aplican la estrategia contra bloques estáticos de datos viejos.
│   └── bin/                 # Herramientas extras de shell o compilados independientes.
├── dashboard/               # Código Frontend (React/TypeScript). Muestra visualmente las señales al usuario usando Vite.
│   ├── src/                 
│   │   ├── hooks/useDashboardStream.ts # Conecta al WS del backend Rust y normaliza el estado de métricas en frontend.
│   │   └── types/wire.ts               # Interfaces locales alineadas con modelos de datos exportados desde Rust.
├── config/                  # Ajustes (default.yaml). Reglas de trailing stop, pesos predefinidos de indicadores y límites de operación.
├── data/                    # Storage volátil y de base de datos generado en runtime (archivos de historial CSV y JSON de calibración).
├── Cargo.toml               # Manifiesto Rust. Declara dependencias como tokio, ethers, axum, sqlx o reqwest.
└── scripts (sh)             # Incluye scripts como start.sh o diagonose.sh vitales en ciclos GitOps y servidores VPS de Linux (Ubuntu).
```

---

## 5. El Workflow o Ciclo de Vida General
1. **Boteando (Boot/Startup):** `main.rs` carga el `.env`, la IP, puertos, claves y verifica el Flag `DRY_RUN` para determinar si es _Paper_ o _Live Trading_.
2. **WebSockets Inician:** Configuran las conexiones L1 a la red de Gamma API en Polymarket buscando "active markets" y suscribiéndose al ticker y orderbook (L2).
3. **Evaluación de Ticks:** Cada vez que Polymarket reporta un nuevo precio, PolyBot procesa la información en milisegundos a través del pipeline: `RTDS` -> `Oracle` -> `Features`.
4. **Machine Learning + Señal (Inference):** `ml_engine` junto con la capa estratégica calculan puntajes y confianza (confidence factor). Si rompe el threshold -> **Emitir Señal de Apertura**.
5. **Risk Check:** ¿Tenemos liquidez ($)? -> ¿Rompimos el Drawdown del día? -> ¿El token tiene el spread correcto? Sí a todo -> Ejecución.
6. **Trailing Loop de Cierre:** El loop de la posición evalúa ticks continuos, aplicando Stop Loss (fijo), y toma utilidades dinámicas cerradas usando el `risk/` en tiempo real.
7. **Feedback y Entrenamiento ML (Calibración):** Se guarda el resultado numérico de la transacción. El bot no solo usa análisis técnico, sino que emplea *Machine Learning* para adaptar sus pesos (weights).
   - El motor `ml_engine` se reentrena periódicamente basándose en el historial de operaciones (almacenado en `data/`).
   - El algoritmo evalúa qué indicadores (Ej: 'EMA Trend', 'RSI') acertaron o fallaron.
   - Si un indicador falló catastróficamente, su influencia (peso) se penaliza. Si acertó de forma constante, su influencia sube.
   - Este aprendizaje se consolida actualizando los archivos de calibración, permitiendo al bot adaptarse automáticamente a las condiciones cambiantes del mercado 15m/1h.

---

## 6. Variables de Entorno y Requirements Esenciales

El archivo `.env` o variables VPS deben contar estrictamente con:
- `PRIVATE_KEY`: Ej: `0xABC...` que operará.
- `POLYMARKET_ADDRESS`: Ej: `0xDEF...` La correspondiente public address fondeada en Polygon.
- `POLYBOT__BOT__DRY_RUN`: `true` para paper trading o validación segura sin riesgo de perder liquidez. Debe usarse tras cada cambio masivo.
- `DASHBOARD_PORT`: Default a 8088.

*(Importante: Todo desarrollo y análisis debe simular pruebas protegiendo estas claves asincrónicamente y nunca revelar trazas de credenciales del usuario de PolyBot en registros, logs de consola, o Git.)*

---

## 7. Instrucciones y Reglas de Oro para un AI Agent
1. **Despliegues con Dashboard (Builds):** Siempre compilar y hacer build en local mediante \`cargo build --release --features dashboard\`. El frontend debe funcionar a la par con el backend. (El `.yml` a veces lo maneja Vercel para el Frontend en Deploy Cloud; respeta directrices dadas si sucede el caso).
2. **Firmas y Decimales Sensibles:** Como Polymarket trabaja asíncrono, los arreglos matemáticos de floats a Decimal a "Integer Strings" o representaciones on-chain deben validar correctamente la precisión. Nunca truncar equivocando cifras centesimales.
3. **Manejo de Rejections (In-Liquidity):** Ya que es un Central Limit Order Book, debes auditar los Spread y Minimum Depths antes de mandar Fill or Kill signals, sino Polymarket tirará errores de liquidez insuficiente. Para modificar cómo liquida o maneja estas excepciones, mira a `config_bridge.rs` o `v3_strategy.rs`.
4. **Operativa remota (VPS):** Actúa como operador DevOps. Conéctate siempre confirmando vía `systemctl status polybot` logs sin colgar la máquina. Solo reinicia tras hacer push/pull a Github usando builds atómicos. En cada cambio, evalúa las consecuencias del bot live.
   - **Comando de Conexión:**
     ```bash
     ssh -i C:\Users\matid\.ssh\polybot_ed25519 -o StrictHostKeyChecking=no root@178.128.224.8
     ```
   - **Logs y Datos de Entrenamiento:** En el VPS residen los logs en vivo (por ejemplo, con `journalctl -u polybot -f`) y la base de datos de entrenamiento del ML (típicamente en `/root/polybot/data/` o en los archivos `.json` de calibración). Analizar estos logs es esencial para entender por qué el ML toma ciertas posiciones.
5. **ML / Risk Logs:** Los prints de error y logging estructurado de `tracing/tracing-subscriber` en Rust son la herramienta del desarrollador principal del Bot. No remuevas advertencias valiosas.

---
_AGENTS.md se generó con la intención de alinear inmediatamente cualquier IA al ciclo de razonamiento del trader algorítmico, y ahorrar horas de investigación._
