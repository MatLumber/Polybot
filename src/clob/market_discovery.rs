//! Market Discovery - Dynamic Market Detection for BTC/ETH Up/Down Markets
//!
//! Automatically discovers and tracks active Polymarket prediction markets
//! for BTC and ETH 15m/1h up/down predictions.

use crate::clob::rest::RestClient;
use crate::clob::types::MarketResponse;
use crate::types::{Asset, Direction, Timeframe};
use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Discovered market with metadata
#[derive(Debug, Clone)]
pub struct DiscoveredMarket {
    pub condition_id: String,
    pub slug: String,
    pub question: String,
    pub end_date: DateTime<Utc>,
    pub active: bool,
    pub closed: bool,
    pub token_ids: Vec<String>,
    pub outcomes: Vec<String>,
    pub outcome_prices: Vec<f64>,
    pub volume: f64,
    pub liquidity: f64,
    pub spread: f64,
    pub asset: Asset,
    pub timeframe: Timeframe,
}

impl DiscoveredMarket {
    /// Get token ID for a specific direction
    pub fn token_for_direction(&self, direction: Direction) -> Option<String> {
        self.outcomes.iter().enumerate().find_map(|(idx, outcome)| {
            let outcome_lower = outcome.to_lowercase();
            let matches = match direction {
                Direction::Up => outcome_lower.contains("up") || outcome_lower.contains("yes"),
                Direction::Down => outcome_lower.contains("down") || outcome_lower.contains("no"),
                _ => false,
            };
            if matches {
                self.token_ids.get(idx).cloned()
            } else {
                None
            }
        })
    }
}

/// Market discovery engine using Gamma API
pub struct MarketDiscovery {
    rest_client: RestClient,
    gamma_url: String,
    /// Currently tracked markets per (Asset, Timeframe)
    tracked_markets: HashMap<(Asset, Timeframe), DiscoveredMarket>,
    /// Last update time
    last_update: Option<DateTime<Utc>>,
    /// Update interval (default: 60 seconds)
    update_interval_secs: u64,
    /// Markets that expired recently (to avoid re-discovery)
    expired_cache: HashMap<String, DateTime<Utc>>,
}

impl MarketDiscovery {
    pub fn new(gamma_url: impl Into<String>) -> Self {
        Self {
            rest_client: RestClient::new("https://clob.polymarket.com", None, None, None, None),
            gamma_url: gamma_url.into(),
            tracked_markets: HashMap::new(),
            last_update: None,
            update_interval_secs: 60,
            expired_cache: HashMap::new(),
        }
    }

    pub fn with_update_interval(mut self, secs: u64) -> Self {
        self.update_interval_secs = secs;
        self
    }

    /// Check if markets need refresh
    pub fn needs_refresh(&self) -> bool {
        match self.last_update {
            None => true,
            Some(last) => {
                let elapsed = Utc::now().signed_duration_since(last).num_seconds();
                elapsed >= self.update_interval_secs as i64
            }
        }
    }

    /// Get time until next refresh
    pub fn seconds_until_refresh(&self) -> i64 {
        match self.last_update {
            None => 0,
            Some(last) => {
                let elapsed = Utc::now().signed_duration_since(last).num_seconds();
                (self.update_interval_secs as i64 - elapsed).max(0)
            }
        }
    }

    /// Discover and update all markets
    pub async fn refresh_markets(&mut self) -> Result<Vec<MarketChange>> {
        let now = Utc::now();
        let mut changes = Vec::new();

        // Clean expired cache (entries older than 1 hour)
        self.expired_cache
            .retain(|_, dt| now.signed_duration_since(*dt).num_hours() < 1);

        // Discover markets for each asset/timeframe combination
        for asset in [Asset::BTC, Asset::ETH] {
            for timeframe in [Timeframe::Min15, Timeframe::Hour1] {
                match self.discover_market(asset, timeframe).await {
                    Ok(Some(new_market)) => {
                        let key = (asset, timeframe);

                        // Check if market changed
                        match self.tracked_markets.get(&key) {
                            Some(existing) => {
                                if existing.condition_id != new_market.condition_id {
                                    info!(
                                        asset = ?asset,
                                        timeframe = ?timeframe,
                                        old_slug = %existing.slug,
                                        new_slug = %new_market.slug,
                                        old_end = %existing.end_date,
                                        new_end = %new_market.end_date,
                                        "🔄 Market rollover detected"
                                    );

                                    // Mark old market as expired
                                    self.expired_cache
                                        .insert(existing.condition_id.clone(), now);

                                    changes.push(MarketChange::Rollover {
                                        asset,
                                        timeframe,
                                        old_market: existing.clone(),
                                        new_market: new_market.clone(),
                                    });
                                } else {
                                    debug!(
                                        asset = ?asset,
                                        timeframe = ?timeframe,
                                        slug = %new_market.slug,
                                        "Market unchanged"
                                    );
                                }
                            }
                            None => {
                                info!(
                                    asset = ?asset,
                                    timeframe = ?timeframe,
                                    slug = %new_market.slug,
                                    end_date = %new_market.end_date,
                                    "🎯 New market discovered"
                                );
                                changes.push(MarketChange::NewMarket {
                                    asset,
                                    timeframe,
                                    market: new_market.clone(),
                                });
                            }
                        }

                        self.tracked_markets.insert(key, new_market);
                    }
                    Ok(None) => {
                        // No active market found
                        let key = (asset, timeframe);
                        if let Some(existing) = self.tracked_markets.get(&key) {
                            // Check if existing market expired
                            if now > existing.end_date {
                                warn!(
                                    asset = ?asset,
                                    timeframe = ?timeframe,
                                    slug = %existing.slug,
                                    end_date = %existing.end_date,
                                    "⚠️ Tracked market has expired, no replacement found"
                                );
                                changes.push(MarketChange::Expired {
                                    asset,
                                    timeframe,
                                    market: existing.clone(),
                                });
                                self.expired_cache
                                    .insert(existing.condition_id.clone(), now);
                                self.tracked_markets.remove(&key);
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            asset = ?asset,
                            timeframe = ?timeframe,
                            error = %e,
                            "Failed to discover market"
                        );
                    }
                }
            }
        }

        self.last_update = Some(now);
        Ok(changes)
    }

    /// Discover a specific market type using Gamma API
    async fn discover_market(
        &mut self,
        asset: Asset,
        timeframe: Timeframe,
    ) -> Result<Option<DiscoveredMarket>> {
        let now = Utc::now();

        // Calculate expected window
        let _window_duration = match timeframe {
            Timeframe::Min15 => chrono::Duration::minutes(15),
            Timeframe::Hour1 => chrono::Duration::hours(1),
        };

        // Look for markets ending in the next 2 hours that match our criteria
        let _lookahead = chrono::Duration::hours(2);

        // Fetch markets from Gamma API
        let markets = self.fetch_candidate_markets(asset, timeframe).await?;

        debug!(
            asset = ?asset,
            timeframe = ?timeframe,
            candidates = markets.len(),
            "Fetched candidate markets"
        );

        // Find the best matching market
        let best_market = markets
            .into_iter()
            .filter(|m| self.is_valid_market(m, asset, timeframe, now))
            .max_by(|a, b| {
                // Prioritize markets that:
                // 1. Have more liquidity
                // 2. End sooner (more immediate relevance)
                // 3. Have valid up/down outcomes
                let score_a = a.liquidity + a.volume * 0.1
                    - (a.end_date.signed_duration_since(now).num_seconds().abs() as f64 * 0.001);
                let score_b = b.liquidity + b.volume * 0.1
                    - (b.end_date.signed_duration_since(now).num_seconds().abs() as f64 * 0.001);
                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

        Ok(best_market)
    }

    /// Fetch candidate markets from Gamma API using /public-search
    /// Searches for up/down markets with queries like "btc updown 15m" or "bitcoin up or down"
    async fn fetch_candidate_markets(
        &self,
        asset: Asset,
        timeframe: Timeframe,
    ) -> Result<Vec<DiscoveredMarket>> {
        let mut all_markets = Vec::new();
        let now = Utc::now();

        // Calculate expected window for this timeframe
        let (window_start, window_end) = match timeframe {
            Timeframe::Min15 => {
                let start = now;
                let end = now + chrono::Duration::hours(2);
                (start, end)
            }
            Timeframe::Hour1 => {
                let start = now;
                let end = now + chrono::Duration::hours(3);
                (start, end)
            }
        };

        // Use events endpoint with date filtering
        info!("🔍 Fetching events for {:?} {:?} (window: {} to {})", 
              asset, timeframe, window_start, window_end);
        
        let events = self
            .rest_client
            .get_events_with_date_filter(
                &self.gamma_url, 
                200, 
                0,
                Some(&now.to_rfc3339()),
                Some(&window_end.to_rfc3339()),
            )
            .await?;
        info!("📊 Events endpoint returned {} events", events.len());

        for event in events {
            let event_slug = event.slug.clone().unwrap_or_default();
            let event_title = &event.title;

            let slug_lower = event_slug.to_lowercase();
            let title_lower = event_title.to_lowercase();

            // Check if asset matches
            let asset_match = match asset {
                Asset::BTC => {
                    slug_lower.contains("btc")
                        || title_lower.contains("btc")
                        || title_lower.contains("bitcoin")
                }
                Asset::ETH => {
                    slug_lower.contains("eth")
                        || title_lower.contains("eth")
                        || title_lower.contains("ethereum")
                }
                _ => false,
            };

            // Check if it's an up/down market
            let is_updown = slug_lower.contains("updown")
                || title_lower.contains("up or down")
                || slug_lower.contains("up-or-down");

            // Log for debugging
            if asset_match && is_updown {
                info!(
                    "🔎 {:?} up/down found: '{}' slug='{}' active={} closed={:?}",
                    asset, event_title, event_slug, event.active, event.closed
                );
            }

            if !asset_match || !is_updown {
                continue;
            }

            if !event.active {
                debug!("⏭️ Skipping '{}': not active", event_title);
                continue;
            }

            // Parse end date
            let end_date = event
                .end_date
                .as_deref()
                .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                .map(|dt| dt.with_timezone(&Utc));

            // Skip if end date is in the past
            if let Some(ed) = end_date {
                if ed <= now {
                    debug!("⏭️ Skipping '{}': already ended", event_title);
                    continue;
                }
            } else {
                debug!("⏭️ Skipping '{}': no end date", event_title);
                continue;
            }

            info!(
                "🎯 Found candidate event: '{}' (slug='{}') active={} ends={}",
                event_title, event_slug, event.active, 
                end_date.map(|d| d.to_string()).unwrap_or_default()
            );

            // Process markets in this event
            for market in event.markets {
                if let Some(discovered) = self.convert_market_response(market, asset, timeframe) {
                    all_markets.push(discovered);
                }
            }
        }

        // Sort by end_date (closest first) and take the best candidate
        all_markets.sort_by(|a, b| a.end_date.cmp(&b.end_date));

        // Keep only the market with the closest end date (the "current" one)
        let result: Vec<DiscoveredMarket> = all_markets.into_iter().take(1).collect();

        info!(
            "🎯 Selected {} current market(s) for {:?} {:?}",
            result.len(),
            asset,
            timeframe
        );
        for m in &result {
            info!("   ✅ {} (ends at {})", m.condition_id, m.end_date);
        }

        Ok(result)
    }

    /// Parse market from JSON search result
    fn parse_market_from_json(
        &self,
        json: &serde_json::Value,
        asset: Asset,
        timeframe: Timeframe,
    ) -> Option<DiscoveredMarket> {
        let condition_id = json.get("conditionId").and_then(|c| c.as_str())?;
        let slug = json.get("slug").and_then(|s| s.as_str()).unwrap_or("");
        let question = json.get("question").and_then(|q| q.as_str())?;
        let active = json
            .get("active")
            .and_then(|a| a.as_bool())
            .unwrap_or(false);
        let closed = json.get("closed").and_then(|c| c.as_bool()).unwrap_or(true);
        let end_date_str = json.get("endDate").and_then(|d| d.as_str())?;

        // Parse end date
        let end_date = DateTime::parse_from_rfc3339(end_date_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))?;

        // Skip if not active or closed
        if !active || closed {
            return None;
        }

        // Parse token IDs from clobTokenIds
        let token_ids: Vec<String> = json
            .get("clobTokenIds")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if token_ids.len() != 2 {
            return None;
        }

        // Parse outcomes
        let outcomes: Vec<String> = json
            .get("outcomes")
            .and_then(|o| o.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if outcomes.len() != 2 {
            return None;
        }

        // Parse outcome prices
        let outcome_prices: Vec<f64> = json
            .get("outcomePrices")
            .and_then(|p| p.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().and_then(|s| s.parse().ok()))
                    .collect()
            })
            .unwrap_or_default();

        let liquidity = json
            .get("liquidityNum")
            .and_then(|l| l.as_f64())
            .unwrap_or(0.0);

        let best_bid = json.get("bestBid").and_then(|b| b.as_f64());
        let best_ask = json.get("bestAsk").and_then(|a| a.as_f64());

        let spread = match (best_bid, best_ask) {
            (Some(bid), Some(ask)) if ask > bid => ask - bid,
            _ => 0.0,
        };

        Some(DiscoveredMarket {
            condition_id: condition_id.to_string(),
            slug: slug.to_string(),
            question: question.to_string(),
            end_date,
            active,
            closed,
            token_ids,
            outcomes,
            outcome_prices,
            volume: 0.0,
            liquidity,
            spread,
            asset,
            timeframe,
        })
    }

    /// Convert MarketResponse to DiscoveredMarket
    fn convert_market_response(
        &self,
        market: MarketResponse,
        asset: Asset,
        expected_timeframe: Timeframe,
    ) -> Option<DiscoveredMarket> {
        let condition_id = &market.condition_id;

        // Parse end date
        let end_date = market
            .end_date
            .as_deref()
            .and_then(|d| {
                DateTime::parse_from_rfc3339(d)
                    .or_else(|_| DateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S%.fZ"))
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1));

        // Parse outcome prices
        let outcome_prices: Vec<f64> = market
            .outcome_prices
            .iter()
            .filter_map(|p| p.parse::<f64>().ok())
            .collect();

        // Build text for analysis
        let text = format!(
            "{} {}",
            market.slug.as_deref().unwrap_or(""),
            market.question
        )
        .to_lowercase();

        // Calculate time until expiry
        let now = Utc::now();
        let duration_to_end = end_date.signed_duration_since(now);
        let minutes_to_end = duration_to_end.num_minutes();

        // Detect timeframe using multiple patterns
        let detected_timeframe = self.detect_timeframe(&text, minutes_to_end)?;

        // Verify timeframe matches what we're looking for
        if detected_timeframe != expected_timeframe {
            debug!(
                "Market {} rejected: timeframe mismatch (detected={:?} expected={:?})",
                condition_id, detected_timeframe, expected_timeframe
            );
            return None;
        }

        // Check token_ids
        if market.clob_token_ids.len() != 2 {
            debug!(
                "Market {} rejected: expected 2 token_ids, got {}",
                condition_id,
                market.clob_token_ids.len()
            );
            return None;
        }

        info!(
            "✅ Market {} ACCEPTED: slug='{}' tf={:?} tokens={} outcomes={:?} ends_in={}min",
            condition_id,
            market.slug.as_deref().unwrap_or(""),
            detected_timeframe,
            market.clob_token_ids.len(),
            market.outcomes,
            minutes_to_end
        );

        Some(DiscoveredMarket {
            condition_id: market.condition_id,
            slug: market.slug.unwrap_or_default(),
            question: market.question,
            end_date,
            active: market.active,
            closed: market.closed.unwrap_or(false),
            token_ids: market.clob_token_ids.clone(),
            outcomes: market.outcomes.clone(),
            outcome_prices,
            volume: 0.0,
            liquidity: market.liquidity_num.unwrap_or(0.0),
            spread: match (market.best_bid, market.best_ask) {
                (Some(bid), Some(ask)) if ask > bid => ask - bid,
                _ => 0.0,
            },
            asset,
            timeframe: detected_timeframe,
        })
    }

    /// Detect timeframe from market text and time to expiry
    fn detect_timeframe(&self, text: &str, minutes_to_end: i64) -> Option<Timeframe> {
        let text_lower = text.to_lowercase();

        // Pattern matching for 15-minute markets
        let is_15m_explicit = text_lower.contains("15m")
            || text_lower.contains("15-minute")
            || text_lower.contains("15 minute")
            || text_lower.contains("min15")
            || text_lower.contains("updown-15m")
            || text_lower.contains("-15m-");

        // Pattern matching for 1-hour markets
        let is_1h_explicit = text_lower.contains("1h")
            || text_lower.contains("1-hour")
            || text_lower.contains("1 hour")
            || text_lower.contains("hourly")
            || text_lower.contains("-1h-")
            || text_lower.contains("updown-1h");

        // Hourly patterns like "2am-et", "3pm-et", etc.
        let has_hour_pattern = text_lower.contains("am-et")
            || text_lower.contains("pm-et")
            || text_lower.contains(":00-et")
            || text_lower.contains(":30-et");

        // Check for up/down market without explicit timeframe
        let is_updown = text_lower.contains("up or down")
            || text_lower.contains("updown")
            || text_lower.contains("up-down");

        // Decision logic
        if is_15m_explicit {
            return Some(Timeframe::Min15);
        }

        if is_1h_explicit || has_hour_pattern {
            return Some(Timeframe::Hour1);
        }

        // For generic up/down markets, infer from time to expiry
        if is_updown {
            // 15m markets typically expire within 30 minutes
            // 1h markets typically expire within 90 minutes
            if minutes_to_end > 0 && minutes_to_end <= 30 {
                return Some(Timeframe::Min15);
            } else if minutes_to_end > 30 && minutes_to_end <= 120 {
                return Some(Timeframe::Hour1);
            } else if minutes_to_end > 0 && minutes_to_end <= 180 {
                // Could still be valid, default to 1h for longer windows
                return Some(Timeframe::Hour1);
            }
        }

        // Fallback: infer from time remaining
        if minutes_to_end > 0 && minutes_to_end <= 25 {
            return Some(Timeframe::Min15);
        } else if minutes_to_end > 25 && minutes_to_end <= 120 {
            return Some(Timeframe::Hour1);
        }

        debug!(
            "Cannot detect timeframe: text='{}' minutes_to_end={}",
            text.chars().take(80).collect::<String>(),
            minutes_to_end
        );
        None
    }

    /// Validate if a market matches our criteria
    fn is_valid_market(
        &self,
        market: &DiscoveredMarket,
        asset: Asset,
        timeframe: Timeframe,
        now: DateTime<Utc>,
    ) -> bool {
        let condition_id = &market.condition_id;

        // Must not be in expired cache
        if self.expired_cache.contains_key(condition_id) {
            debug!("Market {} rejected: in expired cache", condition_id);
            return false;
        }

        // Must be active and not closed
        if !market.active || market.closed {
            debug!(
                "Market {} rejected: active={} closed={}",
                condition_id, market.active, market.closed
            );
            return false;
        }

        // Must not have ended
        if now > market.end_date {
            debug!(
                "Market {} rejected: already ended (end={} now={})",
                condition_id, market.end_date, now
            );
            return false;
        }

        // Must have exactly 2 outcomes/tokens
        if market.token_ids.len() != 2 || market.outcomes.len() != 2 {
            debug!(
                "Market {} rejected: token_ids={} outcomes={}",
                condition_id,
                market.token_ids.len(),
                market.outcomes.len()
            );
            return false;
        }

        // Verify asset match
        let asset_match = match asset {
            Asset::BTC => {
                let text = format!("{} {}", market.slug, market.question).to_lowercase();
                text.contains("btc") || text.contains("bitcoin")
            }
            Asset::ETH => {
                let text = format!("{} {}", market.slug, market.question).to_lowercase();
                text.contains("eth") || text.contains("ethereum")
            }
            _ => false,
        };
        if !asset_match {
            debug!(
                "Market {} rejected: asset mismatch (slug='{}' question='{}')",
                condition_id, market.slug, market.question
            );
            return false;
        }

        // Verify timeframe match
        if market.timeframe != timeframe {
            debug!(
                "Market {} rejected: timeframe mismatch (market={:?} requested={:?})",
                condition_id, market.timeframe, timeframe
            );
            return false;
        }

        // Verify up/down outcomes exist
        let has_up = market.outcomes.iter().any(|o| {
            let ol = o.to_lowercase();
            ol.contains("up") || ol.contains("yes") || ol.contains("higher")
        });
        let has_down = market.outcomes.iter().any(|o| {
            let ol = o.to_lowercase();
            ol.contains("down") || ol.contains("no") || ol.contains("lower")
        });

        if !has_up || !has_down {
            debug!(
                "Market {} rejected: outcomes missing up={} down={} (outcomes={:?})",
                condition_id, has_up, has_down, market.outcomes
            );
            return false;
        }

        // Market should end within reasonable timeframe (not too far in future)
        // Up/down markets can be created with several hours of anticipation
        let time_to_end = market.end_date.signed_duration_since(now);
        let max_lookahead = match timeframe {
            Timeframe::Min15 => chrono::Duration::hours(24),
            Timeframe::Hour1 => chrono::Duration::hours(24),
        };

        if time_to_end > max_lookahead {
            debug!(
                "Market {} rejected: ends too far in future ({} > {:?})",
                condition_id, time_to_end, max_lookahead
            );
            return false;
        }

        info!(
            "Market {} VALIDATED for {:?} {:?}",
            condition_id, asset, timeframe
        );
        true
    }

    /// Get currently tracked markets
    pub fn get_tracked_markets(&self) -> &HashMap<(Asset, Timeframe), DiscoveredMarket> {
        &self.tracked_markets
    }

    /// Get market for a specific asset/timeframe
    pub fn get_market(&self, asset: Asset, timeframe: Timeframe) -> Option<&DiscoveredMarket> {
        self.tracked_markets.get(&(asset, timeframe))
    }

    /// Get all token IDs for WebSocket subscription
    pub fn get_all_token_ids(&self) -> Vec<String> {
        self.tracked_markets
            .values()
            .flat_map(|m| m.token_ids.iter().cloned())
            .collect()
    }

    /// Build token to (Asset, Timeframe, Direction) mapping
    pub fn build_token_map(&self) -> HashMap<String, (Asset, Timeframe, Direction)> {
        let mut map = HashMap::new();
        for ((asset, timeframe), market) in &self.tracked_markets {
            for (idx, outcome) in market.outcomes.iter().enumerate() {
                let outcome_lower = outcome.to_lowercase();
                let direction = if outcome_lower.contains("up") || outcome_lower.contains("yes") {
                    Direction::Up
                } else if outcome_lower.contains("down") || outcome_lower.contains("no") {
                    Direction::Down
                } else {
                    continue;
                };

                if let Some(token_id) = market.token_ids.get(idx) {
                    map.insert(token_id.clone(), (*asset, *timeframe, direction));
                }
            }
        }
        map
    }

    /// Get time until next market expiration
    pub fn time_to_next_expiration(&self) -> Option<chrono::Duration> {
        let now = Utc::now();
        self.tracked_markets
            .values()
            .map(|m| m.end_date.signed_duration_since(now))
            .filter(|d| d.num_seconds() > 0)
            .min()
    }

    /// Check if any market expires within the given duration
    pub fn has_market_expiring_within(&self, duration: chrono::Duration) -> bool {
        let now = Utc::now();
        self.tracked_markets.values().any(|m| {
            let time_left = m.end_date.signed_duration_since(now);
            time_left.num_seconds() > 0 && time_left <= duration
        })
    }

    /// Get last update time
    pub fn last_update(&self) -> Option<DateTime<Utc>> {
        self.last_update
    }
}

/// Represents a change in market state
#[derive(Debug, Clone)]
pub enum MarketChange {
    /// New market discovered
    NewMarket {
        asset: Asset,
        timeframe: Timeframe,
        market: DiscoveredMarket,
    },
    /// Market rolled over to new one
    Rollover {
        asset: Asset,
        timeframe: Timeframe,
        old_market: DiscoveredMarket,
        new_market: DiscoveredMarket,
    },
    /// Market expired with no replacement
    Expired {
        asset: Asset,
        timeframe: Timeframe,
        market: DiscoveredMarket,
    },
}

impl MarketChange {
    pub fn asset(&self) -> Asset {
        match self {
            MarketChange::NewMarket { asset, .. } => *asset,
            MarketChange::Rollover { asset, .. } => *asset,
            MarketChange::Expired { asset, .. } => *asset,
        }
    }

    pub fn timeframe(&self) -> Timeframe {
        match self {
            MarketChange::NewMarket { timeframe, .. } => *timeframe,
            MarketChange::Rollover { timeframe, .. } => *timeframe,
            MarketChange::Expired { timeframe, .. } => *timeframe,
        }
    }

    pub fn requires_reconnect(&self) -> bool {
        matches!(
            self,
            MarketChange::Rollover { .. } | MarketChange::NewMarket { .. }
        )
    }
}
