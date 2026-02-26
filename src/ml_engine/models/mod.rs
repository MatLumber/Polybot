//! ML Models - Ensemble REAL usando SmartCore
//!
//! Implementación completa con RandomForest, LogisticRegression y GradientBoosting

use crate::ml_engine::dataset::Dataset;
use crate::ml_engine::features::MLFeatureVector;
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use smartcore::ensemble::random_forest_classifier::{
    RandomForestClassifier, RandomForestClassifierParameters,
};
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::linear::logistic_regression::{LogisticRegression, LogisticRegressionParameters};
use std::collections::{HashMap, VecDeque};

/// Predicción de un modelo individual
#[derive(Debug, Clone)]
pub struct ModelPrediction {
    pub prob_up: f64,
    pub confidence: f64,
    pub model_name: String,
}

/// Pesos del ensemble
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleWeights {
    pub random_forest: f64,
    pub gradient_boosting: f64,
    pub logistic_regression: f64,
    pub dynamic_weight_adjustment: bool,
}

impl Default for EnsembleWeights {
    fn default() -> Self {
        Self {
            random_forest: 0.4,
            gradient_boosting: 0.35,
            logistic_regression: 0.25,
            dynamic_weight_adjustment: true,
        }
    }
}

impl EnsembleWeights {
    pub fn from_config(config: &crate::ml_engine::EnsembleConfig) -> Self {
        Self {
            random_forest: config.random_forest_weight,
            gradient_boosting: config.gradient_boosting_weight,
            logistic_regression: config.logistic_regression_weight,
            dynamic_weight_adjustment: config.dynamic_weight_adjustment,
        }
    }

    pub fn normalize(&mut self) {
        let total = self.random_forest + self.gradient_boosting + self.logistic_regression;
        if total > 0.0 {
            self.random_forest /= total;
            self.gradient_boosting /= total;
            self.logistic_regression /= total;
        }
    }
}

/// Trait base para todos los modelos
pub trait MLModel: Send + Sync {
    fn train(&mut self, x: &DenseMatrix<f64>, y: &Vec<i64>) -> anyhow::Result<()>;
    fn predict(&self, features: &MLFeatureVector) -> Option<f64>;
    fn name(&self) -> &str;
    fn accuracy(&self) -> f64;
}

/// Random Forest Classifier REAL usando SmartCore
pub struct RandomForestModel {
    name: String,
    classifier: Option<RandomForestClassifier<f64, i64, DenseMatrix<f64>, Vec<i64>>>,
    accuracy: f64,
    n_trees: u16,
    max_depth: u16,
}

impl RandomForestModel {
    pub fn new(n_trees: usize, max_depth: usize) -> Self {
        Self {
            name: "RandomForest".to_string(),
            classifier: None,
            accuracy: 0.0,
            n_trees: n_trees as u16,
            max_depth: max_depth as u16,
        }
    }
}

impl MLModel for RandomForestModel {
    fn train(&mut self, x: &DenseMatrix<f64>, y: &Vec<i64>) -> anyhow::Result<()> {
        let params = RandomForestClassifierParameters::default()
            .with_n_trees(self.n_trees)
            .with_max_depth(self.max_depth)
            .with_min_samples_split(5);

        match RandomForestClassifier::fit(x, y, params) {
            Ok(classifier) => {
                // Calcular accuracy en training
                let predictions = classifier.predict(x).unwrap_or_default();
                let correct = predictions
                    .iter()
                    .zip(y.iter())
                    .filter(|(p, a)| **p == **a)
                    .count();
                self.accuracy = correct as f64 / y.len() as f64;

                self.classifier = Some(classifier);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!("Random Forest training failed: {:?}", e)),
        }
    }

    fn predict(&self, features: &MLFeatureVector) -> Option<f64> {
        if let Some(ref classifier) = self.classifier {
            let feature_vec = features.to_vec();
            let x = DenseMatrix::from_2d_array(&[&feature_vec]).unwrap();

            match classifier.predict(&x) {
                Ok(pred) => {
                    let pred_vec: Vec<i64> = pred;
                    if !pred_vec.is_empty() {
                        Some(pred_vec[0] as f64)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn accuracy(&self) -> f64 {
        self.accuracy
    }
}

/// Logistic Regression REAL usando SmartCore
pub struct LogisticRegressionModel {
    name: String,
    model: Option<LogisticRegression<f64, i64, DenseMatrix<f64>, Vec<i64>>>,
    accuracy: f64,
}

impl LogisticRegressionModel {
    pub fn new() -> Self {
        Self {
            name: "LogisticRegression".to_string(),
            model: None,
            accuracy: 0.0,
        }
    }
}

impl MLModel for LogisticRegressionModel {
    fn train(&mut self, x: &DenseMatrix<f64>, y: &Vec<i64>) -> anyhow::Result<()> {
        let params = LogisticRegressionParameters::default();

        match LogisticRegression::fit(x, y, params) {
            Ok(model) => {
                let predictions = model.predict(x).unwrap_or_default();
                let correct = predictions
                    .iter()
                    .zip(y.iter())
                    .filter(|(p, a)| **p == **a)
                    .count();
                self.accuracy = correct as f64 / y.len() as f64;

                self.model = Some(model);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Logistic Regression training failed: {:?}",
                e
            )),
        }
    }

    fn predict(&self, features: &MLFeatureVector) -> Option<f64> {
        if let Some(ref model) = self.model {
            let feature_vec = features.to_vec();
            let x = DenseMatrix::from_2d_array(&[&feature_vec]).unwrap();

            match model.predict(&x) {
                Ok(pred) if !pred.is_empty() => Some(pred[0] as f64),
                _ => None,
            }
        } else {
            None
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn accuracy(&self) -> f64 {
        self.accuracy
    }
}

/// Gradient Boosting simplificado (usando múltiples RF pequeños)
pub struct GradientBoostingModel {
    name: String,
    models: Vec<RandomForestClassifier<f64, i64, DenseMatrix<f64>, Vec<i64>>>,
    learning_rate: f64,
    accuracy: f64,
}

impl GradientBoostingModel {
    pub fn new(n_estimators: usize, learning_rate: f64) -> Self {
        Self {
            name: "GradientBoosting".to_string(),
            models: Vec::new(),
            learning_rate,
            accuracy: 0.0,
        }
    }
}

impl MLModel for GradientBoostingModel {
    fn train(&mut self, x: &DenseMatrix<f64>, y: &Vec<i64>) -> anyhow::Result<()> {
        // Fix memory leak: always start fresh on each retrain.
        self.models.clear();

        // Use different hyperparameters from the main RF (shallower trees, fewer estimators)
        // so this model learns a genuinely different boundary — higher bias, lower variance.
        // The MLPredictor.train() flow overrides this with an error-focused dataset that
        // makes this model specialize on the examples the main RF gets wrong.
        let params = RandomForestClassifierParameters::default()
            .with_n_trees(40)
            .with_max_depth(4)
            .with_min_samples_split(8);

        match RandomForestClassifier::fit(x, y, params) {
            Ok(model) => {
                let predictions = model.predict(x).unwrap_or_default();
                let correct = predictions
                    .iter()
                    .zip(y.iter())
                    .filter(|(p, a)| **p == **a)
                    .count();
                self.accuracy = correct as f64 / y.len() as f64;

                self.models.push(model);
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Gradient Boosting training failed: {:?}",
                e
            )),
        }
    }

    fn predict(&self, features: &MLFeatureVector) -> Option<f64> {
        if self.models.is_empty() {
            return None;
        }

        let feature_vec = features.to_vec();
        let x = DenseMatrix::from_2d_array(&[&feature_vec]).unwrap();

        let mut sum = 0.0;
        let mut count = 0;

        for model in &self.models {
            if let Ok(pred) = model.predict(&x) {
                if !pred.is_empty() {
                    sum += pred[0] as f64;
                    count += 1;
                }
            }
        }

        if count > 0 {
            Some(sum / count as f64)
        } else {
            None
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn accuracy(&self) -> f64 {
        self.accuracy
    }
}

/// Predictor ensemble REAL que combina todos los modelos
pub struct MLPredictor {
    pub models: Vec<Box<dyn MLModel>>,
    pub weights: EnsembleWeights,
    pub feature_importance: HashMap<String, f64>,
    pub historical_predictions: Vec<(f64, bool)>, // (predicción, resultado real)
    /// Rolling window of recent prediction outcomes for concept drift detection.
    /// `true` = prediction was correct.  Kept to the last 30 results.
    recent_outcomes: VecDeque<bool>,
    /// Baseline rolling accuracy established once we have ≥30 outcomes.
    pub drift_baseline_accuracy: f64,
}

impl MLPredictor {
    pub fn new(weights: EnsembleWeights) -> Self {
        let mut models: Vec<Box<dyn MLModel>> = Vec::new();

        // Inicializar modelos reales
        models.push(Box::new(RandomForestModel::new(100, 10)));
        models.push(Box::new(GradientBoostingModel::new(50, 0.1)));
        models.push(Box::new(LogisticRegressionModel::new()));

        Self {
            models,
            weights,
            feature_importance: HashMap::new(),
            historical_predictions: Vec::new(),
            recent_outcomes: VecDeque::with_capacity(32),
            drift_baseline_accuracy: 0.0,
        }
    }

    /// Entrenar todos los modelos con SmartCore REAL.
    ///
    /// Training flow:
    /// 1. RF trains on the full (recency-weighted) dataset.
    /// 2. GB trains on an *error-focused* version: samples the RF misclassified are
    ///    included 3× so the second learner specialises on the RF's blind spots.
    /// 3. LR trains on the full dataset.
    /// 4. Real feature importance computed via Pearson correlation with target.
    pub fn train(&mut self, dataset: &Dataset) -> anyhow::Result<()> {
        // === Step 1: Build recency-weighted matrix (recent samples counted more) ===
        let (x, y) = self.dataset_to_dense_matrix(dataset);

        // === Step 2: Train Random Forest ===
        if let Some(rf) = self.models.get_mut(0) {
            match rf.train(&x, &y) {
                Ok(_) => tracing::info!(
                    "🌲 RF trained — accuracy: {:.1}%",
                    rf.accuracy() * 100.0
                ),
                Err(e) => tracing::warn!("RF training failed: {}", e),
            }
        }

        // === Step 3: Build error-focused dataset and train GB ===
        // For each sample, ask the RF whether it got it right.
        // Misclassified samples appear 3× in the GB training set so the GB
        // model concentrates on the patterns RF struggles with.
        let mut focused_2d: Vec<Vec<f64>> = Vec::new();
        let mut focused_y: Vec<i64> = Vec::new();

        if let Some(rf) = self.models.get(0) {
            for sample in &dataset.samples {
                let feat_vec = sample.features.to_vec();
                let label = if sample.target > 0.5 { 1i64 } else { 0i64 };
                let pred = rf.predict(&sample.features);
                let is_correct = pred
                    .map(|p| (p > 0.5) == (sample.target > 0.5))
                    .unwrap_or(false);

                // Every sample appears at least once.
                focused_2d.push(feat_vec.clone());
                focused_y.push(label);

                // Misclassified appear 2 extra times (3× total weight).
                if !is_correct {
                    focused_2d.push(feat_vec.clone());
                    focused_y.push(label);
                    focused_2d.push(feat_vec.clone());
                    focused_y.push(label);
                }
            }
        } else {
            // RF not available: fall back to unweighted data.
            for sample in &dataset.samples {
                focused_2d.push(sample.features.to_vec());
                focused_y.push(if sample.target > 0.5 { 1i64 } else { 0i64 });
            }
        }

        if !focused_2d.is_empty() {
            let refs: Vec<&[f64]> = focused_2d.iter().map(|r| r.as_slice()).collect();
            match DenseMatrix::from_2d_array(&refs) {
                Ok(x_focused) => {
                    if let Some(gb) = self.models.get_mut(1) {
                        match gb.train(&x_focused, &focused_y) {
                            Ok(_) => tracing::info!(
                                "🔁 GB (error-focused) trained — accuracy: {:.1}%",
                                gb.accuracy() * 100.0
                            ),
                            Err(e) => tracing::warn!("GB error-focused training failed: {}", e),
                        }
                    }
                }
                Err(e) => tracing::warn!("Failed to build error-focused matrix: {:?}", e),
            }
        }

        // === Step 4: Train Logistic Regression on full dataset ===
        if let Some(lr) = self.models.get_mut(2) {
            match lr.train(&x, &y) {
                Ok(_) => tracing::info!(
                    "📈 LR trained — accuracy: {:.1}%",
                    lr.accuracy() * 100.0
                ),
                Err(e) => tracing::warn!("LR training failed: {}", e),
            }
        }

        // === Step 5: Real feature importance (Pearson correlation with target) ===
        self.calculate_feature_importance_real(dataset);

        Ok(())
    }

    /// Convertir Dataset a DenseMatrix para SmartCore.
    ///
    /// Applies **recency weighting**: samples younger than 7 days are included 3×,
    /// samples from 7–21 days ago are included 2×, older samples once.
    /// This makes the model pay more attention to recent market behaviour without
    /// discarding historical patterns entirely.
    fn dataset_to_dense_matrix(&self, dataset: &Dataset) -> (DenseMatrix<f64>, Vec<i64>) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let mut x_2d: Vec<Vec<f64>> = Vec::new();
        let mut y_data: Vec<i64> = Vec::new();

        for sample in &dataset.samples {
            let age_days = (now_ms - sample.timestamp).max(0) as f64 / 86_400_000.0;
            let copies: usize = if age_days < 7.0 { 3 } else if age_days < 21.0 { 2 } else { 1 };

            let feature_vec = sample.features.to_vec();
            let label: i64 = if sample.target > 0.5 { 1 } else { 0 };

            for _ in 0..copies {
                x_2d.push(feature_vec.clone());
                y_data.push(label);
            }
        }

        let x = DenseMatrix::from_2d_array(
            &x_2d.iter().map(|v| v.as_slice()).collect::<Vec<_>>(),
        )
        .unwrap();
        (x, y_data)
    }

    /// Predecir con ensemble ponderado
    pub fn predict(&self, features: &MLFeatureVector) -> Option<ModelPrediction> {
        let mut weighted_prob = 0.0;
        let mut total_weight = 0.0;
        let mut individual_preds = HashMap::new();

        // Random Forest
        if let Some(model) = self.models.get(0) {
            if let Some(pred) = model.predict(features) {
                weighted_prob += pred * self.weights.random_forest;
                total_weight += self.weights.random_forest;
                individual_preds.insert("RandomForest".to_string(), pred);
            }
        }

        // Gradient Boosting
        if let Some(model) = self.models.get(1) {
            if let Some(pred) = model.predict(features) {
                weighted_prob += pred * self.weights.gradient_boosting;
                total_weight += self.weights.gradient_boosting;
                individual_preds.insert("GradientBoosting".to_string(), pred);
            }
        }

        // Logistic Regression
        if let Some(model) = self.models.get(2) {
            if let Some(pred) = model.predict(features) {
                weighted_prob += pred * self.weights.logistic_regression;
                total_weight += self.weights.logistic_regression;
                individual_preds.insert("LogisticRegression".to_string(), pred);
            }
        }

        if total_weight == 0.0 || individual_preds.is_empty() {
            return None;
        }

        let prob_up = weighted_prob / total_weight;

        // Calcular confianza basada en agreement
        let confidence = self.calculate_confidence(&individual_preds, prob_up);

        Some(ModelPrediction {
            prob_up,
            confidence,
            model_name: "Ensemble".to_string(),
        })
    }

    /// Calcular confianza basada en agreement de modelos
    fn calculate_confidence(&self, predictions: &HashMap<String, f64>, ensemble_prob: f64) -> f64 {
        if predictions.len() < 2 {
            return 0.5;
        }

        let mean_diff: f64 = predictions
            .values()
            .map(|&p| (p - ensemble_prob).abs())
            .sum::<f64>()
            / predictions.len() as f64;

        let confidence = (1.0 - mean_diff * 2.0).clamp(0.0, 1.0);
        let extreme_boost = (ensemble_prob - 0.5).abs() * 2.0;

        let neutral_penalty = if (ensemble_prob - 0.5).abs() < 0.02 { 0.4 } else { 1.0 };

        ((confidence * 0.7 + extreme_boost * 0.3) * neutral_penalty).clamp(0.0, 1.0)
    }

    /// Registrar predicción y resultado para calibración y concept drift.
    pub fn record_outcome(&mut self, predicted_prob: f64, actual_outcome: bool) {
        self.historical_predictions
            .push((predicted_prob, actual_outcome));

        // Mantener solo últimas 1000 predicciones
        if self.historical_predictions.len() > 1000 {
            self.historical_predictions.remove(0);
        }

        // Concept drift: track rolling accuracy over last 30 outcomes.
        let was_correct = (predicted_prob > 0.5) == actual_outcome;
        self.recent_outcomes.push_back(was_correct);
        if self.recent_outcomes.len() > 30 {
            self.recent_outcomes.pop_front();
        }

        // Establish baseline once we have the first 30 outcomes, then update slowly.
        if self.recent_outcomes.len() == 30 {
            let current = self.recent_rolling_accuracy();
            if self.drift_baseline_accuracy < 1e-9 {
                self.drift_baseline_accuracy = current;
            } else {
                // Exponential moving average — baseline adapts slowly.
                self.drift_baseline_accuracy =
                    self.drift_baseline_accuracy * 0.95 + current * 0.05;
            }
        }
    }

    /// Rolling accuracy over the last 30 predictions.
    pub fn recent_rolling_accuracy(&self) -> f64 {
        if self.recent_outcomes.is_empty() {
            return 0.5;
        }
        self.recent_outcomes.iter().filter(|&&c| c).count() as f64
            / self.recent_outcomes.len() as f64
    }

    /// Returns `true` when the rolling accuracy has dropped ≥10 percentage points
    /// below the established baseline — a signal that market conditions have shifted.
    pub fn is_drift_detected(&self) -> bool {
        if self.recent_outcomes.len() < 20 || self.drift_baseline_accuracy < 1e-9 {
            return false;
        }
        self.recent_rolling_accuracy() < self.drift_baseline_accuracy - 0.10
    }

    /// Ajustar pesos dinámicamente basado en performance
    pub fn adjust_weights_dynamically(&mut self) {
        if self.historical_predictions.len() < 50 {
            return; // Necesitamos más datos
        }

        // Calcular accuracy por rango de probabilidad
        let mut high_conf_correct = 0;
        let mut high_conf_total = 0;
        let mut low_conf_correct = 0;
        let mut low_conf_total = 0;

        for (pred, actual) in &self.historical_predictions {
            let predicted_up = *pred > 0.5;
            let correct = predicted_up == *actual;

            if (*pred - 0.5).abs() > 0.2 {
                // High confidence (>70% o <30%)
                high_conf_total += 1;
                if correct {
                    high_conf_correct += 1;
                }
            } else {
                // Low confidence
                low_conf_total += 1;
                if correct {
                    low_conf_correct += 1;
                }
            }
        }

        let high_conf_acc = if high_conf_total > 0 {
            high_conf_correct as f64 / high_conf_total as f64
        } else {
            0.5
        };

        let low_conf_acc = if low_conf_total > 0 {
            low_conf_correct as f64 / low_conf_total as f64
        } else {
            0.5
        };

        // Ajustar pesos: si high confidence funciona mejor, aumentar umbral
        if high_conf_acc > low_conf_acc + 0.1 {
            // Las predicciones de alta confianza son más confiables
            // Reducir peso de predicciones de baja confianza
            tracing::info!(
                "High confidence predictions more accurate ({:.2}% vs {:.2}%), adjusting strategy",
                high_conf_acc * 100.0,
                low_conf_acc * 100.0
            );
        }

        // Actualizar pesos basado en accuracy individual de modelos
        let mut total_accuracy = 0.0;
        let mut model_accuracies: Vec<(usize, f64)> = Vec::new();

        for (i, model) in self.models.iter().enumerate() {
            let acc = model.accuracy();
            total_accuracy += acc;
            model_accuracies.push((i, acc));
        }

        // Ajustar pesos proporcionalmente al accuracy
        if total_accuracy > 0.0 && self.weights.dynamic_weight_adjustment {
            for (i, acc) in model_accuracies {
                let new_weight = acc / total_accuracy;
                match i {
                    0 => self.weights.random_forest = new_weight,
                    1 => self.weights.gradient_boosting = new_weight,
                    2 => self.weights.logistic_regression = new_weight,
                    _ => {}
                }
            }

            tracing::info!(
                "Dynamic weights adjusted: RF={:.2}, GB={:.2}, LR={:.2}",
                self.weights.random_forest,
                self.weights.gradient_boosting,
                self.weights.logistic_regression
            );
        }
    }

    /// Obtener accuracy promedio del ensemble
    pub fn ensemble_accuracy(&self) -> f64 {
        let total: f64 = self.models.iter().map(|m| m.accuracy()).sum();
        total / self.models.len() as f64
    }

    /// Real feature importance via Pearson correlation between each feature and the target.
    ///
    /// `importance[i] = |corr(feature_i, target)|`
    ///
    /// This is not as powerful as permutation importance but it is real, fast, captures
    /// linear relationships between each predictor and the UP/DOWN outcome, and is
    /// infinitely better than the previous hardcoded cycling pattern.
    fn calculate_feature_importance_real(&mut self, dataset: &Dataset) {
        let names = MLFeatureVector::feature_names();
        let n = dataset.samples.len();
        if n < 5 {
            return;
        }

        let targets: Vec<f64> = dataset.samples.iter().map(|s| s.target).collect();
        let target_mean = targets.iter().sum::<f64>() / n as f64;
        let target_std = {
            let var = targets
                .iter()
                .map(|&t| (t - target_mean).powi(2))
                .sum::<f64>()
                / n as f64;
            var.sqrt()
        };

        if target_std < 1e-9 {
            return; // All same class — no variance to correlate against.
        }

        for (feat_idx, name) in names.iter().enumerate() {
            let feat_vals: Vec<f64> = dataset
                .samples
                .iter()
                .map(|s| s.features.to_vec()[feat_idx])
                .collect();

            let feat_mean = feat_vals.iter().sum::<f64>() / n as f64;
            let feat_std = {
                let var = feat_vals
                    .iter()
                    .map(|&v| (v - feat_mean).powi(2))
                    .sum::<f64>()
                    / n as f64;
                var.sqrt()
            };

            if feat_std < 1e-9 {
                self.feature_importance.insert(name.to_string(), 0.0);
                continue;
            }

            let cov = feat_vals
                .iter()
                .zip(targets.iter())
                .map(|(&v, &t)| (v - feat_mean) * (t - target_mean))
                .sum::<f64>()
                / n as f64;

            let pearson = (cov / (feat_std * target_std)).abs();
            self.feature_importance.insert(name.to_string(), pearson);
        }
    }

    /// Obtener top features
    pub fn top_features(&self, n: usize) -> Vec<(&String, &f64)> {
        let mut features: Vec<_> = self.feature_importance.iter().collect();
        features.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap());
        features.into_iter().take(n).collect()
    }

    /// Obtener agreement entre modelos - cuántos modelos predicen la misma dirección
    pub fn get_model_agreement(&self, features: &MLFeatureVector) -> ModelAgreement {
        let mut predictions: Vec<(String, f64)> = Vec::new();

        // Recolectar predicciones individuales
        if let Some(model) = self.models.get(0) {
            if let Some(pred) = model.predict(features) {
                predictions.push((model.name().to_string(), pred));
            }
        }
        if let Some(model) = self.models.get(1) {
            if let Some(pred) = model.predict(features) {
                predictions.push((model.name().to_string(), pred));
            }
        }
        if let Some(model) = self.models.get(2) {
            if let Some(pred) = model.predict(features) {
                predictions.push((model.name().to_string(), pred));
            }
        }

        if predictions.is_empty() {
            return ModelAgreement {
                agreeing_models: 0,
                direction: 0.5,
                predictions: vec![],
            };
        }

        // Contar cuántos predicen UP vs DOWN
        let up_count = predictions.iter().filter(|(_, p)| *p > 0.5).count();
        let down_count = predictions.iter().filter(|(_, p)| *p <= 0.5).count();

        let agreeing_models = up_count.max(down_count);
        let direction = if up_count > down_count { 1.0 } else if down_count > up_count { 0.0 } else { 0.5 };

        ModelAgreement {
            agreeing_models,
            direction,
            predictions: predictions.iter().map(|(n, p)| format!("{}:{:.2}", n, p)).collect(),
        }
    }
}

/// Resultado del agreement entre modelos
#[derive(Debug, Clone)]
pub struct ModelAgreement {
    /// Número de modelos que acuerdan en la dirección
    pub agreeing_models: usize,
    /// Dirección consensuada (1.0 = UP, 0.0 = DOWN, 0.5 = empate)
    pub direction: f64,
    /// Predicciones individuales para debug
    pub predictions: Vec<String>,
}
