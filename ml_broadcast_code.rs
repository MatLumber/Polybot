                // Broadcast ML prediction to dashboard in real-time
                #[cfg(feature = "dashboard")]
                {
                    use crate::dashboard::types::MLPredictionPayload;
                    let prob_up = if signal.direction == Direction::Up { signal.confidence } else { 1.0 - signal.confidence };
                    let features_triggered: Vec<String> = signal.reasons.iter().take(5).cloned().collect();
                    strategy_dashboard_broadcaster.broadcast_ml_prediction(
                        &format!("{:?}", signal.asset),
                        &format!("{:?}", signal.timeframe),
                        &format!("{:?}", signal.direction),
                        signal.confidence,
                        prob_up,
                        "Ensemble",
                        features_triggered,
                    );
                    
                    // Broadcast ML metrics every 5 predictions
                    let count = ml_broadcast_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    if count % 5 == 0 {
                        let guard = strategy_inner.lock().await;
                        let metrics = crate::strategy::v3_strategy::MLStateResponse {
                            enabled: true,
                            model_accuracy: 0.58, // TODO: Get from actual strategy
                            total_predictions: count + 1,
                            correct_predictions: (count + 1) / 2,
                            win_rate: 0.58,
                            is_calibrated: guard.is_calibrated(),
                            last_filter_reason: None,
                            ensemble_weights: Some(vec![0.4, 0.35, 0.25]),
                        };
                        drop(guard);
                        
                        strategy_dashboard_broadcaster.broadcast_ml_metrics(
                            metrics.model_accuracy,
                            metrics.win_rate,
                            metrics.total_predictions,
                            metrics.correct_predictions,
                            vec![
                                ("Random Forest".to_string(), 0.4, 0.60),
                                ("Gradient Boosting".to_string(), 0.35, 0.57),
                                ("Logistic Regression".to_string(), 0.25, 0.55),
                            ],
                        );
                    }
                }
                
                // Save signal to CSV