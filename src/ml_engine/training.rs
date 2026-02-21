//! Training Pipeline - Entrenamiento y walk-forward validation

use crate::ml_engine::dataset::{Dataset, LabeledSample};
use crate::ml_engine::models::{EnsembleWeights, MLPredictor};
use crate::ml_engine::{MLEngineConfig, Prediction};
use serde::{Deserialize, Serialize};

/// Configuración para walk-forward
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkForwardConfig {
    /// Días de entrenamiento
    pub train_days: i64,
    /// Días de test
    pub test_days: i64,
    /// Paso (días)
    pub step_days: i64,
    /// Early stopping patience
    pub early_stopping_patience: usize,
}

impl Default for WalkForwardConfig {
    fn default() -> Self {
        Self {
            train_days: 30,
            test_days: 7,
            step_days: 7,
            early_stopping_patience: 10,
        }
    }
}

/// Pipeline de entrenamiento
pub struct TrainingPipeline {
    config: MLEngineConfig,
    walk_forward_config: WalkForwardConfig,
    /// Historial de métricas por ventana
    pub window_metrics: Vec<WindowMetrics>,
}

/// Métricas de una ventana de walk-forward
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowMetrics {
    pub train_start: i64,
    pub train_end: i64,
    pub test_start: i64,
    pub test_end: i64,
    pub train_size: usize,
    pub test_size: usize,
    pub accuracy: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
}

impl TrainingPipeline {
    pub fn new(config: MLEngineConfig, walk_forward_config: WalkForwardConfig) -> Self {
        Self {
            config,
            walk_forward_config,
            window_metrics: Vec::new(),
        }
    }

    /// Entrenar modelo con todo el dataset
    pub fn train_full(&self, dataset: &Dataset) -> anyhow::Result<MLPredictor> {
        let weights = EnsembleWeights::from_config(&self.config.ensemble);
        let mut predictor = MLPredictor::new(weights);

        // Balancear clases
        let mut balanced_dataset = dataset.clone();
        balanced_dataset.balance_classes();

        // Entrenar
        predictor.train(&balanced_dataset)?;

        Ok(predictor)
    }

    /// Walk-forward validation
    pub fn walk_forward_validation(
        &mut self,
        full_dataset: &Dataset,
    ) -> anyhow::Result<(MLPredictor, Vec<WindowMetrics>)> {
        let mut all_metrics = Vec::new();
        let samples = &full_dataset.samples;

        if samples.is_empty() {
            return Err(anyhow::anyhow!("Dataset vacío"));
        }

        let first_ts = samples.first().unwrap().timestamp;
        let last_ts = samples.last().unwrap().timestamp;

        // Generar ventanas
        let mut current_start = first_ts;
        let train_ms = self.walk_forward_config.train_days * 24 * 60 * 60 * 1000;
        let test_ms = self.walk_forward_config.test_days * 24 * 60 * 60 * 1000;
        let step_ms = self.walk_forward_config.step_days * 24 * 60 * 60 * 1000;

        let mut best_predictor: Option<MLPredictor> = None;
        let mut best_accuracy = 0.0;

        while current_start + train_ms + test_ms <= last_ts {
            let train_start = current_start;
            let train_end = current_start + train_ms;
            let test_start = train_end;
            let test_end = (test_start + test_ms).min(last_ts);

            // Split dataset
            let train_samples: Vec<_> = samples
                .iter()
                .filter(|s| s.timestamp >= train_start && s.timestamp < train_end)
                .cloned()
                .collect();

            let test_samples: Vec<_> = samples
                .iter()
                .filter(|s| s.timestamp >= test_start && s.timestamp < test_end)
                .cloned()
                .collect();

            if train_samples.len() < 20 || test_samples.len() < 5 {
                current_start += step_ms;
                continue;
            }

            let train_dataset = Dataset {
                samples: train_samples,
                trade_samples: Vec::new(),
            };

            let test_dataset = Dataset {
                samples: test_samples,
                trade_samples: Vec::new(),
            };

            // Entrenar
            let weights = EnsembleWeights::from_config(&self.config.ensemble);
            let mut predictor = MLPredictor::new(weights);
            predictor.train(&train_dataset)?;

            // Evaluar
            let metrics = self.evaluate_window(&predictor, &test_dataset);
            let window_metric = WindowMetrics {
                train_start,
                train_end,
                test_start,
                test_end,
                train_size: train_dataset.len(),
                test_size: test_dataset.len(),
                accuracy: metrics.accuracy,
                win_rate: metrics.win_rate,
                profit_factor: metrics.profit_factor,
                sharpe_ratio: metrics.sharpe_ratio,
                max_drawdown: metrics.max_drawdown,
            };

            all_metrics.push(window_metric);

            // Track best predictor
            if metrics.accuracy > best_accuracy {
                best_accuracy = metrics.accuracy;
                best_predictor = Some(predictor);
            }

            current_start += step_ms;
        }

        self.window_metrics = all_metrics.clone();

        if let Some(predictor) = best_predictor {
            // Re-entrenar con todo el dataset al final
            let mut final_predictor = predictor;
            final_predictor.train(full_dataset)?;
            Ok((final_predictor, all_metrics))
        } else {
            Err(anyhow::anyhow!("No se pudo entrenar en ninguna ventana"))
        }
    }

    /// Evaluar métricas de una ventana
    fn evaluate_window(
        &self,
        predictor: &MLPredictor,
        test_dataset: &Dataset,
    ) -> WindowResultMetrics {
        let mut correct = 0;
        let mut wins = 0;
        let mut losses = 0;
        let mut total_pnl = 0.0;
        let mut gross_profit = 0.0;
        let mut gross_loss = 0.0;
        let mut returns = Vec::new();

        for sample in &test_dataset.samples {
            if let Some(prediction) = predictor.predict(&sample.features) {
                let predicted_up = prediction.prob_up > 0.5;
                let actual_up = sample.target > 0.5;

                if predicted_up == actual_up {
                    correct += 1;
                }

                // Simular P&L
                let pnl = if predicted_up == actual_up {
                    wins += 1;
                    let profit = 0.8; // Ganancia típica
                    gross_profit += profit;
                    profit
                } else {
                    losses += 1;
                    let loss: f64 = -1.0; // Pérdida total
                    gross_loss += loss.abs();
                    loss
                };

                total_pnl += pnl;
                returns.push(pnl);
            }
        }

        let total = test_dataset.len() as f64;
        let accuracy = correct as f64 / total;
        let win_rate = if wins + losses > 0 {
            wins as f64 / (wins + losses) as f64
        } else {
            0.5
        };

        let profit_factor = if gross_loss > 0.0 {
            gross_profit / gross_loss
        } else {
            1.0
        };

        let sharpe = self.calculate_sharpe(&returns);
        let drawdown = self.calculate_max_drawdown(&returns);

        WindowResultMetrics {
            accuracy,
            win_rate,
            profit_factor,
            sharpe_ratio: sharpe,
            max_drawdown: drawdown,
        }
    }

    /// Calcular Sharpe ratio
    fn calculate_sharpe(&self, returns: &[f64]) -> f64 {
        if returns.len() < 2 {
            return 0.0;
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;
        let variance =
            returns.iter().map(|&r| (r - mean).powi(2)).sum::<f64>() / returns.len() as f64;
        let std = variance.sqrt();

        if std > 0.0 {
            mean / std
        } else {
            0.0
        }
    }

    /// Calcular máximo drawdown
    fn calculate_max_drawdown(&self, returns: &[f64]) -> f64 {
        let mut peak = 0.0;
        let mut max_dd = 0.0;
        let mut cumsum = 0.0;

        for &ret in returns {
            cumsum += ret;
            if cumsum > peak {
                peak = cumsum;
            }
            let dd = peak - cumsum;
            if dd > max_dd {
                max_dd = dd;
            }
        }

        max_dd
    }

    /// Generar reporte de walk-forward
    pub fn generate_report(&self) -> WalkForwardReport {
        if self.window_metrics.is_empty() {
            return WalkForwardReport::default();
        }

        let avg_accuracy = self.window_metrics.iter().map(|m| m.accuracy).sum::<f64>()
            / self.window_metrics.len() as f64;

        let avg_win_rate = self.window_metrics.iter().map(|m| m.win_rate).sum::<f64>()
            / self.window_metrics.len() as f64;

        let avg_pf = self
            .window_metrics
            .iter()
            .map(|m| m.profit_factor)
            .sum::<f64>()
            / self.window_metrics.len() as f64;

        let avg_sharpe = self
            .window_metrics
            .iter()
            .map(|m| m.sharpe_ratio)
            .sum::<f64>()
            / self.window_metrics.len() as f64;

        WalkForwardReport {
            n_windows: self.window_metrics.len(),
            avg_accuracy,
            avg_win_rate,
            avg_profit_factor: avg_pf,
            avg_sharpe,
            consistency_score: self.calculate_consistency(),
            window_details: self.window_metrics.clone(),
        }
    }

    /// Calcular score de consistencia (menos varianza = mejor)
    fn calculate_consistency(&self) -> f64 {
        if self.window_metrics.len() < 2 {
            return 0.5;
        }

        let accuracies: Vec<f64> = self.window_metrics.iter().map(|m| m.accuracy).collect();

        let mean = accuracies.iter().sum::<f64>() / accuracies.len() as f64;
        let variance =
            accuracies.iter().map(|&a| (a - mean).powi(2)).sum::<f64>() / accuracies.len() as f64;

        // Score inversamente proporcional a varianza
        (1.0 - variance.sqrt() * 2.0).clamp(0.0, 1.0)
    }
}

/// Métricas de resultado de ventana
#[derive(Debug, Clone)]
struct WindowResultMetrics {
    pub accuracy: f64,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub sharpe_ratio: f64,
    pub max_drawdown: f64,
}

/// Reporte de walk-forward
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WalkForwardReport {
    pub n_windows: usize,
    pub avg_accuracy: f64,
    pub avg_win_rate: f64,
    pub avg_profit_factor: f64,
    pub avg_sharpe: f64,
    pub consistency_score: f64,
    pub window_details: Vec<WindowMetrics>,
}

impl WalkForwardReport {
    pub fn print(&self) {
        println!("\n╔══════════════════════════════════════════╗");
        println!("║      WALK-FORWARD VALIDATION REPORT      ║");
        println!("╚══════════════════════════════════════════╝\n");

        println!("Ventanas evaluadas: {}", self.n_windows);
        println!("Accuracy promedio:  {:.2}%", self.avg_accuracy * 100.0);
        println!("Win Rate promedio:  {:.2}%", self.avg_win_rate * 100.0);
        println!("Profit Factor:      {:.2}", self.avg_profit_factor);
        println!("Sharpe Ratio:       {:.2}", self.avg_sharpe);
        println!("Consistencia:       {:.2}%", self.consistency_score * 100.0);

        println!("\nDetalle por ventana:");
        println!(
            "{:<6} {:<8} {:<8} {:<10} {:<10}",
            "#", "Train", "Test", "Accuracy", "Win Rate"
        );
        println!("{}", "-".repeat(50));

        for (i, window) in self.window_details.iter().enumerate() {
            println!(
                "{:<6} {:<8} {:<8} {:<10.2} {:<10.2}",
                i + 1,
                window.train_size,
                window.test_size,
                window.accuracy * 100.0,
                window.win_rate * 100.0
            );
        }

        // Verdict
        println!("\n{}", "=".repeat(50));
        if self.avg_win_rate > 0.55 {
            println!("✅ RESULTADO: Modelo viable para trading");
        } else if self.avg_win_rate > 0.52 {
            println!("⚠️  RESULTADO: Marginal - requiere mejoras");
        } else {
            println!("❌ RESULTADO: No viable - necesita más trabajo");
        }
        println!("{}", "=".repeat(50));
    }
}
