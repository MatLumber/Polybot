#!/bin/bash
# Update PolyBot ML v3.0 en VPS
# Ejecutar en tu VPS después de hacer git pull

echo "🔄 Actualizando PolyBot ML v3.0..."
echo ""

# 1. Detener el bot si está corriendo
echo "1. Deteniendo bot si está corriendo..."
pkill -f "polybot" || true
sleep 2

# 2. Compilar nueva versión
echo "2. Compilando nueva versión..."
cargo build --release 2>&1 | tail -10

if [ $? -ne 0 ]; then
    echo "❌ Error en compilación"
    exit 1
fi

echo "✅ Compilación exitosa"
echo ""

# 3. Verificar que la config tiene V3 activado
echo "3. Verificando configuración..."
if ! grep -q "use_v3_strategy: true" config/local.yaml; then
    echo "⚠️  Activando V3 Strategy en config..."
    sed -i 's/use_v3_strategy: false/use_v3_strategy: true/' config/local.yaml
fi

echo "✅ Configuración OK"
echo ""

# 4. Mostrar instrucciones
echo "==================================="
echo "✅ ACTUALIZACIÓN COMPLETADA"
echo "==================================="
echo ""
echo "Para iniciar el bot:"
echo "  ./target/release/polybot"
echo ""
echo "Dashboard disponible en:"
echo "  http://tu-vps:3000"
echo ""
echo "WebSocket (tiempo real):"
echo "  ws://tu-vps:3000/ws"
echo ""
echo "Endpoints ML:"
echo "  http://tu-vps:3000/api/ml/state"
echo "  http://tu-vps:3000/api/ml/metrics"
echo "  http://tu-vps:3000/api/ml/models"
echo ""
echo "Ahora el Dashboard ML se actualiza en TIEMPO REAL! 🚀"
echo ""
