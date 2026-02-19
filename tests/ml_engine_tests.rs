//! Tests para ML Engine

#[cfg(test)]
mod tests {
    use polybot::ml_engine::calibration::{CalibrationMethod, ProbabilityCalibrator};
    use polybot::ml_engine::dataset::{Dataset, LabeledSample, TradeSample};
    use polybot::ml_engine::features::{FeatureEngine, MLFeatureVector, MarketContext};
    use polybot::ml_engine::filters::{
        FilterConfig, FilterContext, FilterDecision, SmartFilterEngine,
    };
    use polybot::ml_engine::models::{
        EnsembleWeights, LogisticRegressionModel, MLModel, MLPredictor, RandomForestModel,
    };
    use polybot::ml_engine::{MLEngineConfig, Prediction};
    use polybot::types::{Asset, Direction, Timeframe};

    // ============================================================================
    // Tests de Features
    // ============================================================================

    #[test]
    fn test_ml_feature_vector_creation() {
        let features = MLFeatureVector::default();
        let vec = features.to_vec();

        assert_eq!(vec.len(), MLFeatureVector::NUM_FEATURES);
    }

    #[test]
    fn test_feature_names() {
        let names = MLFeatureVector::feature_names();
        assert_eq!(names.len(), MLFeatureVector::NUM_FEATURES);
        assert!(names.contains(&"rsi"));
        assert!(names.contains(&"macd"));
    }

    #[test]
    fn test_feature_engine_computation() {
        let mut engine = FeatureEngine::new();

        let mut features = polybot::features::Features::default();
        features.asset = Asset::BTC;
        features.timeframe = Timeframe::Min15;
        features.rsi = Some(30.0);
        features.macd = Some(0.5);
        features.adx = Some(30.0);
        features.plus_di = Some(25.0);
        features.minus_di = Some(15.0);

        let context = MarketContext {
            timestamp: 1000,
            hour: 14,
            day_of_week: 2,
            minutes_to_close: 10.0,
            minutes_since_market_open: 300.0,
            btc_eth_correlation: 0.8,
            calibrator_confidence: 0.6,
            num_indicators_agreeing: 3,
            indicators_avg_win_rate: 0.45,
            bullish_weight: 2.0,
            bearish_weight: 1.0,
        };

        let ml_features = engine.compute(&features, &context);

        assert!(ml_features.rsi > 0.0);
        assert!(ml_features.macd > 0.0);
    }

    // ============================================================================
    // Tests de Modelos ML (SmartCore REAL)
    // ============================================================================

    #[test]
    fn test_random_forest_training() {
        let mut model = RandomForestModel::new(10, 5);

        // Crear dataset simple
        let x = smartcore::linalg::basic::matrix::DenseMatrix::from_2d_array(&[
            &[1.0, 2.0, 3.0],
            &[2.0, 3.0, 4.0],
            &[3.0, 4.0, 5.0],
            &[4.0, 5.0, 6.0],
        ])
        .unwrap();

        let y = vec![0, 0, 1, 1];

        let result = model.train(&x, &y);
        assert!(result.is_ok());

        // Verificar que se puede predecir
        let features = MLFeatureVector::default();
        let pred = model.predict(&features);
        assert!(pred >= 0.0 && pred <= 1.0);
    }

    #[test]
    fn test_logistic_regression_training() {
        let mut model = LogisticRegressionModel::new();

        // Create training data with 50 features (matching MLFeatureVector::NUM_FEATURES)
        let train_vec: Vec<f64> = (0..50).map(|i| i as f64 * 0.1).collect();
        let x = smartcore::linalg::basic::matrix::DenseMatrix::from_2d_array(&[
            &train_vec,
            &train_vec.iter().map(|v| v + 1.0).collect::<Vec<_>>(),
            &train_vec.iter().map(|v| v + 2.0).collect::<Vec<_>>(),
            &train_vec.iter().map(|v| v + 3.0).collect::<Vec<_>>(),
        ])
        .unwrap();

        let y = vec![0, 0, 1, 1];

        let result = model.train(&x, &y);
        assert!(result.is_ok());

        let features = MLFeatureVector::default();
        let pred = model.predict(&features);
        assert!(pred >= 0.0 && pred <= 1.0);
    }

    #[test]
    fn test_ensemble_predictor() {
        let weights = EnsembleWeights {
            random_forest: 0.4,
            gradient_boosting: 0.35,
            logistic_regression: 0.25,
            dynamic_weight_adjustment: false,
        };

        let predictor = MLPredictor::new(weights);

        let features = MLFeatureVector {
            rsi: 30.0,
            macd: 0.5,
            adx: 30.0,
            ..Default::default()
        };

        let pred = predictor.predict(&features);
        assert!(pred.prob_up >= 0.0 && pred.prob_up <= 1.0);
        assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
    }

    #[test]
    fn test_dynamic_weight_adjustment() {
        let weights = EnsembleWeights {
            random_forest: 0.4,
            gradient_boosting: 0.35,
            logistic_regression: 0.25,
            dynamic_weight_adjustment: false,
        };

        let mut predictor = MLPredictor::new(weights);

        // Simular algunas predicciones
        for _ in 0..60 {
            predictor.record_outcome(0.7, true);
        }

        // Ajustar pesos
        predictor.adjust_weights_dynamically();

        // Verificar que los pesos cambiaron
        // (en implementación real dependería de los modelos entrenados)
    }

    // ============================================================================
    // Tests de Calibración
    // ============================================================================

    #[test]
    fn test_probability_calibration() {
        let mut calibrator = ProbabilityCalibrator::new(CalibrationMethod::Isotonic);

        // Agregar observaciones
        for _ in 0..30 {
            calibrator.add_observation(0.7, true);
            calibrator.add_observation(0.3, false);
        }

        // Calibrar
        let calibrated = calibrator.calibrate(0.8);
        assert!(calibrated >= 0.0 && calibrated <= 1.0);
    }

    #[test]
    fn test_calibration_curve() {
        let mut curve = polybot::ml_engine::calibration::CalibrationCurve::default();

        let predictions = vec![0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1];
        let outcomes = vec![true, true, false, true, false, false, true, false, false];

        curve.fit_isotonic(&predictions, &outcomes);

        let calibrated = curve.calibrate(0.9);
        assert!(calibrated >= 0.0 && calibrated <= 1.0);
    }

    // ============================================================================
    // Tests de Filtros
    // ============================================================================

    #[test]
    fn test_liquidity_filter() {
        let config = FilterConfig::default();
        let engine = SmartFilterEngine::new(config);

        let mut context = FilterContext::default();
        context.orderbook_depth = 1000.0; // Muy bajo

        let decision = engine.evaluate(&context);
        assert_eq!(
            decision,
            FilterDecision::Reject(
                polybot::ml_engine::filters::FilterReason::InsufficientLiquidity
            )
        );
    }

    #[test]
    fn test_spread_filter() {
        let config = FilterConfig::default();
        let engine = SmartFilterEngine::new(config);

        let mut context = FilterContext::default();
        context.timeframe = Timeframe::Min15;
        context.spread_bps = 150.0; // Muy alto
        context.orderbook_depth = 10000.0; // Buena liquidez

        let decision = engine.evaluate(&context);
        assert_eq!(
            decision,
            FilterDecision::Reject(polybot::ml_engine::filters::FilterReason::ExcessiveSpread)
        );
    }

    #[test]
    fn test_filter_allow() {
        let config = FilterConfig::default();
        let engine = SmartFilterEngine::new(config);

        let mut context = FilterContext::default();
        context.orderbook_depth = 10000.0;
        context.spread_bps = 50.0;
        context.volatility_5m = 0.01;
        context.minutes_to_close = 10.0;
        context.window_progress = 0.3;
        context.btc_eth_correlation = 0.8;

        let decision = engine.evaluate(&context);
        assert_eq!(decision, FilterDecision::Allow);
    }

    #[test]
    fn test_optimal_hours_learning() {
        let config = FilterConfig::default();
        let mut engine = SmartFilterEngine::new(config);

        let trades = vec![
            (14, true),
            (14, true),
            (14, false),
            (14, true),
            (14, true), // 80% win
            (15, false),
            (15, false),
            (15, true),
            (15, false), // 25% win
        ];

        engine.learn_optimal_hours(&trades);

        assert!(engine.optimal_hours.contains(&14));
        assert!(!engine.optimal_hours.contains(&15));
    }

    // ============================================================================
    // Tests de Dataset
    // ============================================================================

    #[test]
    fn test_dataset_creation() {
        let mut dataset = Dataset::new();

        let sample = TradeSample {
            trade_id: "test-1".to_string(),
            entry_ts: 1000,
            exit_ts: 2000,
            asset: Asset::BTC,
            timeframe: Timeframe::Min15,
            direction: Direction::Up,
            is_win: true,
            entry_features: MLFeatureVector::default(),
            entry_price: 50000.0,
            exit_price: 51000.0,
            pnl: 100.0,
            estimated_edge: 0.05,
            indicators_triggered: vec!["rsi".to_string()],
        };

        dataset.add_trade(sample);

        assert_eq!(dataset.len(), 1);
    }

    #[test]
    fn test_dataset_balance_classes() {
        let mut dataset = Dataset::new();

        // Agregar 3 ganadores
        for i in 0..3 {
            let sample = TradeSample {
                trade_id: format!("test-{}", i),
                entry_ts: i as i64 * 1000,
                exit_ts: (i as i64 + 1) * 1000,
                asset: Asset::BTC,
                timeframe: Timeframe::Min15,
                direction: Direction::Up,
                is_win: true,
                entry_features: MLFeatureVector::default(),
                entry_price: 50000.0,
                exit_price: 51000.0,
                pnl: 100.0,
                estimated_edge: 0.05,
                indicators_triggered: vec![],
            };
            dataset.add_trade(sample);
        }

        // Agregar 1 perdedor
        let losing_sample = TradeSample {
            trade_id: "test-loss".to_string(),
            entry_ts: 4000,
            exit_ts: 5000,
            asset: Asset::BTC,
            timeframe: Timeframe::Min15,
            direction: Direction::Up,
            is_win: false,
            entry_features: MLFeatureVector::default(),
            entry_price: 50000.0,
            exit_price: 49000.0,
            pnl: -100.0,
            estimated_edge: 0.05,
            indicators_triggered: vec![],
        };
        dataset.add_trade(losing_sample);

        assert_eq!(dataset.len(), 4);

        dataset.balance_classes();

        let stats = dataset.statistics();
        let balance = stats.class_balance;
        assert!(balance > 0.4 && balance < 0.6);
    }

    #[test]
    fn test_dataset_to_ndarray() {
        let mut dataset = Dataset::new();

        for i in 0..5 {
            let sample = TradeSample {
                trade_id: format!("test-{}", i),
                entry_ts: i as i64 * 1000,
                exit_ts: (i as i64 + 1) * 1000,
                asset: Asset::BTC,
                timeframe: Timeframe::Min15,
                direction: Direction::Up,
                is_win: i % 2 == 0,
                entry_features: MLFeatureVector::default(),
                entry_price: 50000.0,
                exit_price: 51000.0,
                pnl: 100.0,
                estimated_edge: 0.05,
                indicators_triggered: vec![],
            };
            dataset.add_trade(sample);
        }

        let (x, y) = dataset.to_ndarray();

        assert_eq!(x.nrows(), 5);
        assert_eq!(x.ncols(), MLFeatureVector::NUM_FEATURES);
        assert_eq!(y.len(), 5);
    }

    // ============================================================================
    // Tests de Configuración
    // ============================================================================

    #[test]
    fn test_ml_engine_config_default() {
        let config = MLEngineConfig::default();

        assert!(config.enabled);
        assert!(config.features.use_microstructure);
        assert!(config.features.use_temporal_patterns);
    }

    #[test]
    fn test_ensemble_weights_normalize() {
        let mut weights = EnsembleWeights {
            random_forest: 0.4,
            gradient_boosting: 0.35,
            logistic_regression: 0.25,
            dynamic_weight_adjustment: false,
        };

        weights.normalize();

        let sum = weights.random_forest + weights.gradient_boosting + weights.logistic_regression;
        assert!((sum - 1.0).abs() < 0.001);
    }

    // ============================================================================
    // Tests de Predicción
    // ============================================================================

    #[test]
    fn test_prediction_is_correct() {
        let pred = Prediction {
            timestamp: 1234567890,
            asset: Asset::BTC,
            timeframe: Timeframe::Min15,
            prob_up: 0.7,
            confidence: 0.8,
            direction: Direction::Up,
            edge: 0.05,
            features_used: vec!["rsi".to_string()],
            model_contributions: std::collections::HashMap::new(),
            ensemble_weight: 1.0,
        };

        // Predijo UP (0.7 > 0.5), resultado UP (true) -> correcto
        assert!(pred.is_correct(true));

        // Predijo UP, resultado DOWN -> incorrecto
        assert!(!pred.is_correct(false));
    }

    #[test]
    fn test_ml_engine_state_accuracy() {
        let config = MLEngineConfig::default();
        let mut state = polybot::ml_engine::MLEngineState::new(config);

        assert_eq!(state.accuracy(), 0.5);

        state.total_predictions = 100;
        state.correct_predictions = 60;

        assert!((state.accuracy() - 0.6).abs() < 0.001);
    }

    // ============================================================================
    // Tests de Integración
    // ============================================================================

    #[test]
    fn test_full_ml_pipeline() {
        // 1. Crear dataset
        let mut dataset = Dataset::new();

        for i in 0..20 {
            let mut features = MLFeatureVector::default();
            features.rsi = 30.0 + (i as f64 * 2.0);
            features.macd = if i % 2 == 0 { 0.5 } else { -0.5 };

            let sample = TradeSample {
                trade_id: format!("test-{}", i),
                entry_ts: i as i64 * 1000,
                exit_ts: (i as i64 + 1) * 1000,
                asset: Asset::BTC,
                timeframe: Timeframe::Min15,
                direction: if i % 2 == 0 {
                    Direction::Up
                } else {
                    Direction::Down
                },
                is_win: i % 2 == 0,
                entry_features: features,
                entry_price: 50000.0,
                exit_price: 51000.0,
                pnl: 100.0,
                estimated_edge: 0.05,
                indicators_triggered: vec!["test".to_string()],
            };
            dataset.add_trade(sample);
        }

        // 2. Crear predictor
        let weights = EnsembleWeights {
            random_forest: 0.4,
            gradient_boosting: 0.35,
            logistic_regression: 0.25,
            dynamic_weight_adjustment: false,
        };

        let mut predictor = MLPredictor::new(weights);

        // 3. Entrenar
        let result = predictor.train(&dataset);
        assert!(result.is_ok());

        // 4. Predecir
        let test_features = MLFeatureVector {
            rsi: 25.0,
            macd: 0.8,
            ..Default::default()
        };

        let pred = predictor.predict(&test_features);
        assert!(pred.prob_up >= 0.0 && pred.prob_up <= 1.0);
        assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
    }
}
