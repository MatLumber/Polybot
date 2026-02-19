//! Dataset - Manejo de datasets para training

use crate::ml_engine::features::MLFeatureVector;
use crate::types::{Asset, Direction, Timeframe};
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};

/// Muestra etiquetada para entrenamiento
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabeledSample {
    /// Vector de features
    pub features: MLFeatureVector,
    /// Target: 1.0 si subió, 0.0 si bajó
    pub target: f64,
    /// Timestamp
    pub timestamp: i64,
    /// Asset
    pub asset: Asset,
    /// Timeframe
    pub timeframe: Timeframe,
    /// Metadata adicional
    pub metadata: SampleMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SampleMetadata {
    /// Precio de entrada
    pub entry_price: f64,
    /// Precio de salida (resolución)
    pub exit_price: f64,
    /// Retorno real
    pub actual_return: f64,
    /// P&L en USDC
    pub pnl: f64,
    /// Indicadores usados por el sistema anterior
    pub indicators_used: Vec<String>,
    /// Confianza del sistema anterior
    pub old_confidence: f64,
}

/// Muestra extraída de trade histórico
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSample {
    /// ID del trade
    pub trade_id: String,
    /// Timestamp de entrada
    pub entry_ts: i64,
    /// Timestamp de salida
    pub exit_ts: i64,
    /// Asset
    pub asset: Asset,
    /// Timeframe
    pub timeframe: Timeframe,
    /// Dirección predicha
    pub direction: Direction,
    /// Resultado: true = win, false = loss
    pub is_win: bool,
    /// Features en momento de entrada
    pub entry_features: MLFeatureVector,
    /// Precio de entrada
    pub entry_price: f64,
    /// Precio de salida (Chainlink)
    pub exit_price: f64,
    /// P&L
    pub pnl: f64,
    /// Edge estimado al entrar
    pub estimated_edge: f64,
    /// Lista de indicadores que generaron señal
    pub indicators_triggered: Vec<String>,
}

/// Dataset completo para ML
#[derive(Debug, Clone)]
pub struct Dataset {
    /// Muestras etiquetadas
    pub samples: Vec<LabeledSample>,
    /// Muestras de trades (metadata)
    pub trade_samples: Vec<TradeSample>,
}

impl Dataset {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            trade_samples: Vec::new(),
        }
    }

    pub fn from_trade_samples(trades: Vec<TradeSample>) -> Self {
        let samples = trades
            .iter()
            .map(|trade| LabeledSample {
                features: trade.entry_features.clone(),
                target: if trade.is_win { 1.0 } else { 0.0 },
                timestamp: trade.entry_ts,
                asset: trade.asset,
                timeframe: trade.timeframe,
                metadata: SampleMetadata {
                    entry_price: trade.entry_price,
                    exit_price: trade.exit_price,
                    actual_return: (trade.exit_price - trade.entry_price) / trade.entry_price,
                    pnl: trade.pnl,
                    indicators_used: trade.indicators_triggered.clone(),
                    old_confidence: trade.entry_features.calibrator_confidence,
                },
            })
            .collect();

        Self {
            samples,
            trade_samples: trades,
        }
    }

    /// Agregar muestra
    pub fn add_sample(&mut self, sample: LabeledSample) {
        self.samples.push(sample);
    }

    /// Agregar trade
    pub fn add_trade(&mut self, trade: TradeSample) {
        self.trade_samples.push(trade.clone());
        self.samples.push(LabeledSample {
            features: trade.entry_features,
            target: if trade.is_win { 1.0 } else { 0.0 },
            timestamp: trade.entry_ts,
            asset: trade.asset,
            timeframe: trade.timeframe,
            metadata: SampleMetadata {
                entry_price: trade.entry_price,
                exit_price: trade.exit_price,
                actual_return: (trade.exit_price - trade.entry_price) / trade.entry_price,
                pnl: trade.pnl,
                indicators_used: trade.indicators_triggered,
                old_confidence: 0.0,
            },
        });
    }

    /// Tamaño del dataset
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Split temporal (no aleatorio) para walk-forward
    pub fn temporal_split(&self, train_ratio: f64) -> (Dataset, Dataset) {
        let split_idx = (self.samples.len() as f64 * train_ratio) as usize;

        let train_samples = self.samples[..split_idx].to_vec();
        let test_samples = self.samples[split_idx..].to_vec();

        (
            Dataset {
                samples: train_samples,
                trade_samples: Vec::new(),
            },
            Dataset {
                samples: test_samples,
                trade_samples: Vec::new(),
            },
        )
    }

    /// Split por tiempo (días)
    pub fn split_by_time(&self, train_days: i64) -> (Dataset, Dataset) {
        if self.samples.is_empty() {
            return (Dataset::new(), Dataset::new());
        }

        let first_ts = self.samples.first().unwrap().timestamp;
        let cutoff_ts = first_ts + train_days * 24 * 60 * 60 * 1000;

        let train: Vec<_> = self
            .samples
            .iter()
            .filter(|s| s.timestamp < cutoff_ts)
            .cloned()
            .collect();
        let test: Vec<_> = self
            .samples
            .iter()
            .filter(|s| s.timestamp >= cutoff_ts)
            .cloned()
            .collect();

        (
            Dataset {
                samples: train,
                trade_samples: Vec::new(),
            },
            Dataset {
                samples: test,
                trade_samples: Vec::new(),
            },
        )
    }

    /// Convertir a ndarray para modelos ML
    pub fn to_ndarray(&self) -> (Array2<f64>, Array1<f64>) {
        let n_samples = self.samples.len();
        let n_features = MLFeatureVector::NUM_FEATURES;

        let mut x = Array2::zeros((n_samples, n_features));
        let mut y = Array1::zeros(n_samples);

        for (i, sample) in self.samples.iter().enumerate() {
            let feature_vec = sample.features.to_vec();
            for (j, &val) in feature_vec.iter().enumerate() {
                x[[i, j]] = val;
            }
            y[i] = sample.target;
        }

        (x, y)
    }

    /// Filtrar por asset
    pub fn filter_by_asset(&self, asset: Asset) -> Dataset {
        let samples: Vec<_> = self
            .samples
            .iter()
            .filter(|s| s.asset == asset)
            .cloned()
            .collect();

        Dataset {
            samples,
            trade_samples: Vec::new(),
        }
    }

    /// Filtrar por timeframe
    pub fn filter_by_timeframe(&self, tf: Timeframe) -> Dataset {
        let samples: Vec<_> = self
            .samples
            .iter()
            .filter(|s| s.timeframe == tf)
            .cloned()
            .collect();

        Dataset {
            samples,
            trade_samples: Vec::new(),
        }
    }

    /// Balancear clases (oversampling de clase minoritaria)
    pub fn balance_classes(&mut self) {
        let n_up = self.samples.iter().filter(|s| s.target == 1.0).count();
        let n_down = self.samples.iter().filter(|s| s.target == 0.0).count();

        if n_up == 0 || n_down == 0 {
            return;
        }

        let minority_class = if n_up < n_down { 1.0 } else { 0.0 };
        let majority_count = n_up.max(n_down);
        let minority_count = n_up.min(n_down);

        let minority_samples: Vec<_> = self
            .samples
            .iter()
            .filter(|s| s.target == minority_class)
            .cloned()
            .collect();

        // Replicar muestras minoritarias
        let needed = majority_count - minority_count;
        for i in 0..needed {
            let sample = minority_samples[i % minority_samples.len()].clone();
            self.samples.push(sample);
        }

        // Shuffle
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        self.samples.shuffle(&mut rng);
    }

    /// Calcular estadísticas del dataset
    pub fn statistics(&self) -> DatasetStats {
        let n = self.samples.len() as f64;
        let n_up = self.samples.iter().filter(|s| s.target == 1.0).count() as f64;

        let avg_features: Vec<f64> = (0..MLFeatureVector::NUM_FEATURES)
            .map(|i| {
                self.samples
                    .iter()
                    .map(|s| s.features.to_vec()[i])
                    .sum::<f64>()
                    / n
            })
            .collect();

        DatasetStats {
            total_samples: self.samples.len(),
            up_samples: n_up as usize,
            down_samples: (n - n_up) as usize,
            class_balance: n_up / n,
            avg_features,
        }
    }

    /// Guardar en disco
    pub fn save(&self, path: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string(&self.samples)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Cargar desde disco
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let samples: Vec<LabeledSample> = serde_json::from_str(&json)?;
        Ok(Dataset {
            samples,
            trade_samples: Vec::new(),
        })
    }
}

/// Estadísticas del dataset
#[derive(Debug, Clone)]
pub struct DatasetStats {
    pub total_samples: usize,
    pub up_samples: usize,
    pub down_samples: usize,
    pub class_balance: f64,
    pub avg_features: Vec<f64>,
}
