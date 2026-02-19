#!/bin/bash
# PolyBot ML v3.0 - Script de inicio rápido

echo "🤖 PolyBot ML v3.0 - Iniciando..."
echo ""

# Verificar que existe el config
if [ ! -f "config/local.yaml" ]; then
    echo "⚠️  No se encontró config/local.yaml"
    echo "📝 Copiando configuración ML..."
    cp config/v3.yaml.example config/local.yaml
    echo "✅ Configuración creada"
    echo ""
fi

# Verificar dependencias
echo "📦 Verificando dependencias..."
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo no instalado. Instala desde https://rustup.rs/"
    exit 1
fi

echo "🔨 Compilando PolyBot ML..."
cargo build --release 2>&1 | tail -5

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Compilación exitosa!"
    echo ""
    echo "🚀 Iniciando PolyBot ML v3.0..."
    echo ""
    echo "📊 Dashboard disponible en: http://localhost:3000"
    echo "📡 WebSocket: ws://localhost:3000/ws"
    echo ""
    echo "🔌 Endpoints ML:"
    echo "   - GET http://localhost:3000/api/ml/state"
    echo "   - GET http://localhost:3000/api/ml/metrics"
    echo "   - GET http://localhost:3000/api/ml/models"
    echo "   - GET http://localhost:3000/api/ml/features"
    echo "   - GET http://localhost:3000/api/ml/training"
    echo ""
    echo "⚙️  Configuración: config/local.yaml"
    echo ""
    
    # Ejecutar
    ./target/release/polybot
else
    echo "❌ Error en la compilación"
    exit 1
fi
