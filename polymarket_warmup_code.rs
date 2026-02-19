// COMPLETE POLYMARKET WARMUP CODE
// This code goes inside the `else` block for native_only mode, after the native_targets array
// and replaces the incomplete `for (slug, asset, timeframe) in native_targets {` loop

            for (slug, asset, timeframe) in native_targets {
                // Find tradeable market for this asset/timeframe
                match feature_clob_client.find_tradeable_market_for_signal(asset, timeframe).await {
                    Some(market) => {
                        if let Some(token_id) = market.tokens.as_ref().and_then(|t| t.get(0)).cloned() {
                            tracing::info!(
                                asset = ?asset,
                                timeframe = ?timeframe,
                                token_id = %token_id,
                                "Fetching Polymarket price history for warmup"
                            );
                            
                            // Try to fetch price history with multiple interval fallbacks
                            let points = match feature_clob_client
                                .get_token_price_history(&token_id, PriceHistoryInterval::Max, None, None, None)
                                .await
                            {
                                Ok(pts) => pts,
                                Err(primary_err) => {
                                    tracing::warn!(
                                        asset = ?asset,
                                        timeframe = ?timeframe,
                                        error = %primary_err,
                                        "Primary interval failed, trying OneWeek"
                                    );
                                    match feature_clob_client
                                        .get_token_price_history(&token_id, PriceHistoryInterval::OneWeek, None, None, None)
                                        .await
                                    {
                                        Ok(pts) => pts,
                                        Err(week_err) => {
                                            tracing::warn!(
                                                asset = ?asset,
                                                timeframe = ?timeframe,
                                                error = %week_err,
                                                "OneWeek interval failed, trying OneDay"
                                            );
                                            match feature_clob_client
                                                .get_token_price_history(&token_id, PriceHistoryInterval::OneDay, None, None, None)
                                                .await
                                            {
                                                Ok(pts) => pts,
                                                Err(day_err) => {
                                                    tracing::warn!(
                                                        asset = ?asset,
                                                        timeframe = ?timeframe,
                                                        error = %day_err,
                                                        "All intervals failed for price history"
                                                    );
                                                    continue;
                                                }
                                            }
                                        }
                                    }
                                }
                            };
                            
                            if points.is_empty() {
                                tracing::warn!(
                                    asset = ?asset,
                                    timeframe = ?timeframe,
                                    "No price history points returned"
                                );
                                continue;
                            }
                            
                            // Convert MarketPrice points to synthetic candles
                            let mut prob_points: Vec<(i64, f64)> = points
                                .into_iter()
                                .filter_map(|point| {
                                    if !point.p.is_finite() || point.p <= 0.0 {
                                        return None;
                                    }
                                    Some((point.t, point.p.clamp(0.0001, 0.9999)))
                                })
                                .collect();
                            
                            prob_points.sort_by_key(|(ts, _)| *ts);
                            prob_points.dedup_by_key(|(ts, _)| *ts);
                            
                            if prob_points.len() < 2 {
                                tracing::warn!(
                                    asset = ?asset,
                                    timeframe = ?timeframe,
                                    points_count = prob_points.len(),
                                    "Not enough valid price points"
                                );
                                continue;
                            }
                            
                            // Build synthetic candles from probability points
                            let candles = build_candles_from_prob_points(asset, timeframe, &prob_points);
                            
                            if candles.len() >= 30 {
                                candle_builder.seed_history(candles.clone());
                                tracing::info!(
                                    asset = ?asset,
                                    timeframe = ?timeframe,
                                    candle_count = candles.len(),
                                    "Successfully seeded {} candles from Polymarket history",
                                    candles.len()
                                );
                            } else {
                                tracing::warn!(
                                    asset = ?asset,
                                    timeframe = ?timeframe,
                                    candle_count = candles.len(),
                                    "Not enough candles built from Polymarket history (need 30+)"
                                );
                            }
                        } else {
                            tracing::warn!(
                                asset = ?asset,
                                timeframe = ?timeframe,
                                "Market found but no token_id available"
                            );
                        }
                    }
                    None => {
                        tracing::warn!(
                            asset = ?asset,
                            timeframe = ?timeframe,
                            "No tradeable market found for warmup"
                        );
                    }
                }
            }
            
            tracing::info!("Polymarket historical data warmup complete");

// Helper function to build candles from probability points
fn build_candles_from_prob_points(
    asset: Asset,
    timeframe: Timeframe,
    prob_points: &[(i64, f64)],
) -> Vec<crate::types::Candle> {
    use crate::types::Candle;
    use crate::oracle::candle_start;
    
    let mut candles = Vec::new();
    if prob_points.len() < 2 {
        return candles;
    }
    
    let anchor_price = 50000.0; // BTC anchor
    let (first_ts, first_p) = prob_points[0];
    let mut prev_prob = first_p.max(0.0001);
    let mut synthetic_price = anchor_price.max(1.0);
    
    // Group points into candles based on timeframe
    let window_ms = timeframe.duration_secs() as i64 * 1000;
    let mut current_window = (first_ts / window_ms) * window_ms;
    let mut window_prices: Vec<f64> = vec![synthetic_price];
    let mut window_ts = first_ts;
    
    for (ts, prob) in prob_points.iter().skip(1) {
        let raw_ret = (*prob / prev_prob) - 1.0;
        let scaled_ret = (raw_ret * 0.35).clamp(-0.03, 0.03);
        synthetic_price = (synthetic_price * (1.0 + scaled_ret)).max(0.01);
        
        let point_window = (*ts / window_ms) * window_ms;
        
        if point_window != current_window && !window_prices.is_empty() {
            // Create candle from window_prices
            let open = window_prices[0];
            let close = window_prices[window_prices.len() - 1];
            let high = window_prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let low = window_prices.iter().cloned().fold(f64::INFINITY, f64::min);
            
            candles.push(Candle {
                open_time: current_window,
                close_time: current_window + window_ms - 1,
                asset,
                timeframe,
                open,
                high,
                low,
                close,
                volume: 0.0,
                trades: window_prices.len() as u64,
            });
            
            current_window = point_window;
            window_prices.clear();
        }
        
        window_prices.push(synthetic_price);
        prev_prob = prob.max(0.0001);
    }
    
    // Create final candle if there are remaining prices
    if !window_prices.is_empty() {
        let open = window_prices[0];
        let close = window_prices[window_prices.len() - 1];
        let high = window_prices.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let low = window_prices.iter().cloned().fold(f64::INFINITY, f64::min);
        
        candles.push(Candle {
            open_time: current_window,
            close_time: current_window + window_ms - 1,
            asset,
            timeframe,
            open,
            high,
            low,
            close,
            volume: 0.0,
            trades: window_prices.len() as u64,
        });
    }
    
    candles
}
