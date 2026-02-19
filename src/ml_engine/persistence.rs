//! ML Persistence - Sistema de persistencia para ML Engine
//!
//! Guarda y recupera todo el estado del ML para sobrevivir reinicios

use crate::ml_engine::dataset::Dataset;
use crate::ml_engine::models::{EnsembleWeights, MLPredictor};
use crate::ml_engine::{MLEngineConfig, MLEngineState};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tracing::{error, info, warn};

/// Estado persistente completo del ML Engine
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MLPersistenceState {
    pub version: String,
    pub config: MLEngineConfig,
    pub ensemble_weights: EnsembleWeights,
    pub total_predictions: usize,
    pub correct_predictions: usize,
    pub last_retraining: Option<i64>,
    pub feature_importance: HashMap<String, f64>,
    pub model_performances: Vec<ModelPerformance>,
    pub training_history: Vec<TrainingRecord>,
    pub saved_at: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelPerformance {
    pub model_name: String,
    pub accuracy: f64,
    pub predictions_count: usize,
    pub correct_count: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingRecord {
    pub timestamp: i64,
    pub samples_count: usize,
    pub accuracy: f64,
    pub ensemble_weights: EnsembleWeights,
}

impl Default for MLPersistenceState {
    fn default() -> Self {
        Self {
            version: "3.0".to_string(),
            config: MLEngineConfig::default(),
            ensemble_weights: EnsembleWeights::default(),
            total_predictions: 0,
            correct_predictions: 0,
            last_retraining: None,
            feature_importance: HashMap::new(),
            model_performances: Vec::new(),
            training_history: Vec::new(),
            saved_at: 0,
        }
    }
}

impl MLPersistenceState {
    pub fn new(config: MLEngineConfig) -> Self {
        Self {
            version: "3.0".to_string(),
            config,
            ensemble_weights: EnsembleWeights::default(),
            total_predictions: 0,
            correct_predictions: 0,
            last_retraining: None,
            feature_importance: HashMap::new(),
            model_performances: Vec::new(),
            training_history: Vec::new(),
            saved_at: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn save(&self, path: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        info!("💾 ML state saved to {}", path);
        Ok(())
    }

    pub fn load(path: &str) -> anyhow::Result<Self> {
        let json = fs::read_to_string(path)?;
        let state: Self = serde_json::from_str(&json)?;
        info!(
            "📂 ML state loaded from {} (version {})",
            path, state.version
        );
        Ok(state)
    }

    pub fn exists(path: &str) -> bool {
        Path::new(path).exists()
    }
}

/// Manager de persistencia del ML
pub struct MLPersistenceManager {
    data_dir: String,
    state_file: String,
    dataset_file: String,
    models_dir: String,
    auto_save_interval: usize,
    predictions_since_save: usize,
}

impl MLPersistenceManager {
    pub fn new(data_dir: &str) -> Self {
        let data_path = Path::new(data_dir);
        let ml_dir = data_path.join("ml_engine");
        let models_dir = ml_dir.join("models");

        // Crear directorios si no existen
        let _ = fs::create_dir_all(&ml_dir);
        let _ = fs::create_dir_all(&models_dir);

        Self {
            data_dir: data_dir.to_string(),
            state_file: ml_dir.join("ml_state.json").to_string_lossy().to_string(),
            dataset_file: ml_dir.join("dataset.json").to_string_lossy().to_string(),
            models_dir: models_dir.to_string_lossy().to_string(),
            auto_save_interval: 10, // Guardar cada 10 predicciones
            predictions_since_save: 0,
        }
    }

    /// Guardar estado completo del ML
    pub fn save_ml_state(
        &mut self,
        predictor: &MLPredictor,
        state: &MLEngineState,
        dataset: &Dataset,
    ) -> anyhow::Result<()> {
        // Guardar estado principal
        let persist_state = MLPersistenceState {
            version: "3.0".to_string(),
            config: state.config.clone(),
            ensemble_weights: predictor.weights.clone(),
            total_predictions: state.total_predictions,
            correct_predictions: state.correct_predictions,
            last_retraining: state.last_retraining,
            feature_importance: predictor.feature_importance.clone(),
            model_performances: predictor
                .models
                .iter()
                .map(|m| ModelPerformance {
                    model_name: m.name().to_string(),
                    accuracy: m.accuracy(),
                    predictions_count: 0,
                    correct_count: 0,
                })
                .collect(),
            training_history: Vec::new(), // Se actualizaría desde training pipeline
            saved_at: chrono::Utc::now().timestamp_millis(),
        };

        persist_state.save(&self.state_file)?;

        // Guardar dataset
        dataset.save(&self.dataset_file)?;

        self.predictions_since_save = 0;
        info!(
            "💾 ML state auto-saved (predictions: {})",
            state.total_predictions
        );

        Ok(())
    }

    /// Cargar estado del ML
    pub fn load_ml_state(&self) -> anyhow::Result<Option<(MLPersistenceState, Dataset)>> {
        if !MLPersistenceState::exists(&self.state_file) {
            info!("📂 No previous ML state found, starting fresh");
            return Ok(None);
        }

        let state = MLPersistenceState::load(&self.state_file)?;

        // Intentar cargar dataset
        let dataset = if Path::new(&self.dataset_file).exists() {
            match Dataset::load(&self.dataset_file) {
                Ok(d) => {
                    info!("📊 Loaded {} samples from dataset", d.len());
                    d
                }
                Err(e) => {
                    warn!("Failed to load dataset: {}, starting empty", e);
                    Dataset::new()
                }
            }
        } else {
            Dataset::new()
        };

        info!(
            "🧠 ML state restored: {} predictions, {:.1}% accuracy",
            state.total_predictions,
            if state.total_predictions > 0 {
                (state.correct_predictions as f64 / state.total_predictions as f64) * 100.0
            } else {
                0.0
            }
        );

        Ok(Some((state, dataset)))
    }

    /// Registrar predicción y guardar si es necesario
    pub fn record_prediction(
        &mut self,
        predictor: &MLPredictor,
        state: &MLEngineState,
        dataset: &Dataset,
    ) -> anyhow::Result<()> {
        self.predictions_since_save += 1;

        if self.predictions_since_save >= self.auto_save_interval {
            self.save_ml_state(predictor, state, dataset)?;
        }

        Ok(())
    }

    /// Guardar métricas de entrenamiento
    pub fn save_training_record(&self, record: TrainingRecord) -> anyhow::Result<()> {
        let training_file = Path::new(&self.data_dir)
            .join("ml_engine")
            .join("training_history.json");

        let mut history: Vec<TrainingRecord> = if training_file.exists() {
            let json = fs::read_to_string(&training_file)?;
            serde_json::from_str(&json).unwrap_or_default()
        } else {
            Vec::new()
        };

        history.push(record);

        // Mantener solo últimos 100 registros
        if history.len() > 100 {
            history.remove(0);
        }

        let json = serde_json::to_string_pretty(&history)?;
        fs::write(training_file, json)?;

        Ok(())
    }

    /// Cargar historial de entrenamiento
    pub fn load_training_history(&self) -> anyhow::Result<Vec<TrainingRecord>> {
        let training_file = Path::new(&self.data_dir)
            .join("ml_engine")
            .join("training_history.json");

        if !training_file.exists() {
            return Ok(Vec::new());
        }

        let json = fs::read_to_string(training_file)?;
        let history: Vec<TrainingRecord> = serde_json::from_str(&json)?;
        Ok(history)
    }

    /// Backup del estado actual
    pub fn backup(&self) -> anyhow::Result<String> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_dir = Path::new(&self.data_dir)
            .join("ml_engine")
            .join("backups")
            .join(timestamp.to_string());

        fs::create_dir_all(&backup_dir)?;

        // Copiar archivos actuales
        let backup_state = backup_dir.join("ml_state.json");
        let backup_dataset = backup_dir.join("dataset.json");

        if Path::new(&self.state_file).exists() {
            fs::copy(&self.state_file, backup_state)?;
        }
        if Path::new(&self.dataset_file).exists() {
            fs::copy(&self.dataset_file, backup_dataset)?;
        }

        let backup_path = backup_dir.to_string_lossy().to_string();
        info!("💾 ML state backed up to {}", backup_path);

        Ok(backup_path)
    }

    /// Listar backups disponibles
    pub fn list_backups(&self) -> anyhow::Result<Vec<String>> {
        let backups_dir = Path::new(&self.data_dir).join("ml_engine").join("backups");

        if !backups_dir.exists() {
            return Ok(Vec::new());
        }

        let mut backups: Vec<String> = fs::read_dir(backups_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();

        backups.sort_by(|a, b| b.cmp(a)); // Más reciente primero

        Ok(backups)
    }

    /// Restaurar desde backup
    pub fn restore_backup(&self, backup_name: &str) -> anyhow::Result<()> {
        let backup_dir = Path::new(&self.data_dir)
            .join("ml_engine")
            .join("backups")
            .join(backup_name);

        if !backup_dir.exists() {
            return Err(anyhow::anyhow!("Backup {} not found", backup_name));
        }

        let backup_state = backup_dir.join("ml_state.json");
        let backup_dataset = backup_dir.join("dataset.json");

        if backup_state.exists() {
            fs::copy(&backup_state, &self.state_file)?;
        }
        if backup_dataset.exists() {
            fs::copy(&backup_dataset, &self.dataset_file)?;
        }

        info!("🔄 ML state restored from backup: {}", backup_name);
        Ok(())
    }
}
