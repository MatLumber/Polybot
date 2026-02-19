//! Calibration - Calibración de probabilidades
//!
//! Convierte probabilidades "raw" del modelo en probabilidades reales
//! usando técnicas como Platt Scaling e Isotonic Regression

use serde::{Deserialize, Serialize};

/// Curva de calibración (mapeo de raw -> calibrated)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CalibrationCurve {
    /// Puntos de la curva (raw_prob, calibrated_prob)
    pub points: Vec<(f64, f64)>,
    /// Número de muestras usadas para calibrar
    pub n_samples: usize,
}

impl CalibrationCurve {
    /// Calibrar una probabilidad raw
    pub fn calibrate(&self, raw_prob: f64) -> f64 {
        if self.points.is_empty() {
            return raw_prob.clamp(0.01, 0.99);
        }

        // Interpolación lineal
        let clamped = raw_prob.clamp(0.0, 1.0);

        // Encontrar punto anterior y siguiente
        for i in 0..self.points.len() - 1 {
            let (x1, y1) = self.points[i];
            let (x2, y2) = self.points[i + 1];

            if clamped >= x1 && clamped <= x2 {
                // Interpolar
                let t = (clamped - x1) / (x2 - x1);
                return (y1 + t * (y2 - y1)).clamp(0.01, 0.99);
            }
        }

        // Si está fuera de rango, usar el extremo más cercano
        if clamped <= self.points[0].0 {
            self.points[0].1
        } else {
            self.points.last().unwrap().1
        }
    }

    /// Ajustar curva usando Isotonic Regression simplificada
    pub fn fit_isotonic(&mut self, predictions: &[f64], outcomes: &[bool]) {
        assert_eq!(predictions.len(), outcomes.len());

        if predictions.is_empty() {
            return;
        }

        // Crear bins por probabilidad
        let n_bins = 10;
        let mut bins: Vec<Vec<bool>> = vec![Vec::new(); n_bins];

        for (pred, outcome) in predictions.iter().zip(outcomes.iter()) {
            let bin_idx = ((pred * n_bins as f64) as usize).min(n_bins - 1);
            bins[bin_idx].push(*outcome);
        }

        // Calcular frecuencia real por bin
        self.points.clear();
        for (i, bin) in bins.iter().enumerate() {
            if !bin.is_empty() {
                let raw_prob = (i as f64 + 0.5) / n_bins as f64;
                let actual_prob = bin.iter().filter(|&&o| o).count() as f64 / bin.len() as f64;
                self.points.push((raw_prob, actual_prob));
            }
        }

        // Asegurar monotonicidad (isotonic)
        self.enforce_monotonicity();

        self.n_samples = predictions.len();
    }

    /// Ajustar usando Platt Scaling (regresión logística)
    pub fn fit_platt(&mut self, predictions: &[f64], outcomes: &[bool]) {
        // Implementación simplificada - en producción usar optimización iterativa
        // Para ahora, usar versión lineal simple
        let n = predictions.len() as f64;
        let mean_pred = predictions.iter().sum::<f64>() / n;
        let mean_outcome = outcomes.iter().filter(|&&o| o).count() as f64 / n;

        // Ajuste lineal simple
        let slope = if mean_pred > 0.0 && mean_pred < 1.0 {
            (mean_outcome - 0.5) / (mean_pred - 0.5)
        } else {
            1.0
        };

        let intercept = mean_outcome - slope * mean_pred;

        // Crear puntos de calibración
        self.points.clear();
        for i in 0..=10 {
            let raw = i as f64 / 10.0;
            let calibrated = (slope * raw + intercept).clamp(0.0, 1.0);
            self.points.push((raw, calibrated));
        }

        self.n_samples = predictions.len();
    }

    /// Forzar monotonicidad (no decreciente)
    fn enforce_monotonicity(&mut self) {
        if self.points.len() < 2 {
            return;
        }

        // Algoritmo PAVA (Pool Adjacent Violators)
        let mut i = 0;
        while i < self.points.len() - 1 {
            if self.points[i].1 > self.points[i + 1].1 {
                // Violación encontrada - hacer promedio
                let avg = (self.points[i].1 + self.points[i + 1].1) / 2.0;
                self.points[i].1 = avg;
                self.points[i + 1].1 = avg;
                // Volver atrás si es necesario
                if i > 0 {
                    i -= 1;
                }
            } else {
                i += 1;
            }
        }
    }

    /// Calcular Expected Calibration Error (ECE)
    pub fn expected_calibration_error(&self) -> f64 {
        if self.points.is_empty() {
            return 0.0;
        }

        // Error promedio entre raw y calibrated
        self.points
            .iter()
            .map(|(raw, cal)| (raw - cal).abs())
            .sum::<f64>()
            / self.points.len() as f64
    }
}

/// Calibrador de probabilidades
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbabilityCalibrator {
    /// Curva de calibración
    pub curve: CalibrationCurve,
    /// Método usado
    pub method: CalibrationMethod,
    /// Historial de predicciones para recalibración
    prediction_history: Vec<(f64, bool)>,
    /// Máximo historial a mantener
    max_history: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CalibrationMethod {
    Isotonic,
    Platt,
    None,
}

impl Default for CalibrationMethod {
    fn default() -> Self {
        CalibrationMethod::Isotonic
    }
}

impl ProbabilityCalibrator {
    pub fn new(method: CalibrationMethod) -> Self {
        Self {
            curve: CalibrationCurve::default(),
            method,
            prediction_history: Vec::with_capacity(1000),
            max_history: 1000,
        }
    }

    /// Calibrar una probabilidad
    pub fn calibrate(&self, raw_prob: f64) -> f64 {
        match self.method {
            CalibrationMethod::Isotonic | CalibrationMethod::Platt => {
                self.curve.calibrate(raw_prob)
            }
            CalibrationMethod::None => raw_prob.clamp(0.01, 0.99),
        }
    }

    /// Agregar resultado para recalibración
    pub fn add_observation(&mut self, predicted_prob: f64, actual_outcome: bool) {
        self.prediction_history
            .push((predicted_prob, actual_outcome));

        if self.prediction_history.len() > self.max_history {
            self.prediction_history.remove(0);
        }

        // Recalibrar cada 50 observaciones
        if self.prediction_history.len() % 50 == 0 {
            self.recalibrate();
        }
    }

    /// Recalibrar usando historial actual
    pub fn recalibrate(&mut self) {
        if self.prediction_history.len() < 30 {
            return;
        }

        let predictions: Vec<f64> = self.prediction_history.iter().map(|(p, _)| *p).collect();
        let outcomes: Vec<bool> = self.prediction_history.iter().map(|(_, o)| *o).collect();

        match self.method {
            CalibrationMethod::Isotonic => {
                self.curve.fit_isotonic(&predictions, &outcomes);
            }
            CalibrationMethod::Platt => {
                self.curve.fit_platt(&predictions, &outcomes);
            }
            CalibrationMethod::None => {}
        }
    }

    /// Calcular métrica de calibración
    pub fn calibration_error(&self) -> f64 {
        self.curve.expected_calibration_error()
    }

    /// Número de observaciones
    pub fn n_observations(&self) -> usize {
        self.prediction_history.len()
    }

    /// Limpiar historial
    pub fn clear_history(&mut self) {
        self.prediction_history.clear();
    }
}

impl Default for ProbabilityCalibrator {
    fn default() -> Self {
        Self::new(CalibrationMethod::Isotonic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calibration_curve() {
        let mut curve = CalibrationCurve::default();

        // Datos de ejemplo: modelo sobreconfiado
        let predictions = vec![0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1];
        let outcomes = vec![true, true, false, true, false, false, true, false, false];

        curve.fit_isotonic(&predictions, &outcomes);

        // Verificar que calibra
        let calibrated = curve.calibrate(0.9);
        assert!(calibrated < 0.9); // Debe reducir sobreconfianza
    }

    #[test]
    fn test_monotonicity() {
        let mut curve = CalibrationCurve::default();
        curve.points = vec![
            (0.0, 0.1),
            (0.2, 0.3),
            (0.4, 0.25), // Violación!
            (0.6, 0.7),
            (0.8, 0.9),
        ];

        curve.enforce_monotonicity();

        // Verificar monotonicidad
        for i in 0..curve.points.len() - 1 {
            assert!(curve.points[i].1 <= curve.points[i + 1].1);
        }
    }
}
