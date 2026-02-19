#!/bin/bash
# Diagnóstico de PolyBot - Verificar por qué no hay señales

echo "🔍 DIAGNÓSTICO DE POLYBOT"
echo "=========================="
echo ""

# 1. Verificar si el proceso está corriendo
echo "1. Verificando si PolyBot está corriendo..."
if pgrep -f "polybot" > /dev/null; then
    echo "✅ PolyBot está corriendo"
    ps aux | grep polybot | grep -v grep
else
    echo "❌ PolyBot NO está corriendo"
fi
echo ""

# 2. Verificar logs recientes
echo "2. Últimos logs (últimas 20 líneas)..."
if [ -f "polybot.log" ]; then
    tail -20 polybot.log | grep -E "(Signal|Position|Paper|Trade|generated|ML)"
else
    echo "No se encontró polybot.log"
fi
echo ""

# 3. Verificar archivos CSV
echo "3. Archivos de trades/signals..."
ls -lh data/*.csv 2>/dev/null | head -10
echo ""

# 4. Verificar configuración
echo "4. Configuración actual..."
echo "Paper Trading: $(grep "enabled:" config/local.yaml | head -1)"
echo "V3 Strategy: $(grep "use_v3_strategy:" config/local.yaml | head -1)"
echo "Assets: $(grep -A2 "assets:" config/local.yaml | head -3)"
echo ""

# 5. Probar endpoints
echo "5. Probando endpoints del dashboard..."
echo "Testing http://localhost:3000/api/stats..."
curl -s http://localhost:3000/api/stats | jq '.data' 2>/dev/null || echo "❌ No responde"
echo ""

echo "Testing http://localhost:3000/api/positions..."
curl -s http://localhost:3000/api/positions | jq '.data' 2>/dev/null || echo "❌ No responde"
echo ""

echo "Testing http://localhost:3000/api/ml/state..."
curl -s http://localhost:3000/api/ml/state | jq '.data' 2>/dev/null || echo "❌ No responde"
echo ""

# 6. Verificar si hay datos en CSV
echo "6. Datos en CSV..."
echo "Trades recientes:"
if [ -f "data/trades_$(date +%Y-%m-%d).csv" ]; then
    wc -l "data/trades_$(date +%Y-%m-%d).csv"
else
    echo "No hay trades CSV de hoy"
fi

echo ""
echo "Signals recientes:"
if [ -f "data/signals_$(date +%Y-%m-%d).csv" ]; then
    wc -l "data/signals_$(date +%Y-%m-%d).csv"
else
    echo "No hay signals CSV de hoy"
fi

echo ""
echo "=========================="
echo "📊 Diagnóstico completo"
