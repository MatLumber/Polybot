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
            rest_client: RestClient::new(
                "https://clob.polymarket.com",
                None, None, None, None,
            ),
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
                                    self.expired_cache.insert(
                                        existing.condition_id.clone(),
                                        now,
                                    );
                                    
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
                score_a.partial_cmp(&score_b).unwrap_or(std::cmp::Ordering::Equal)
            });

        Ok(best_market)
    }

    /// Fetch candidate markets from Gamma API
    async fn fetch_candidate_markets(
        &self,
        asset: Asset,
        _timeframe: Timeframe,
    ) -> Result<Vec<DiscoveredMarket>> {
        // Search tags for up/down markets - these rotate every 15m/1h
        let search_tags: Vec<&str> = match asset {
            Asset::BTC => vec!["Bitcoin", "BTC", "updown", "up-down", "Crypto", "crypto"],
            Asset::ETH => vec!["Ethereum", "ETH", "updown", "up-down", "Crypto", "crypto"],
            _ => return Ok(Vec::new()),
        };

        let mut all_markets = Vec::new();

        // Search using different tags
        for tag in search_tags {
            info!("Fetching markets with tag '{}' for {:?}", tag, asset);
            let markets = self.rest_client
                .get_markets_page(&self.gamma_url, Some(tag), 100, 0)
                .await?;
            
            info!("Tag '{}' returned {} markets", tag, markets.len());
            
            for market in markets {
                let condition_id = market.condition_id.clone();
                let q_preview: String = market.question.chars().take(60).collect();
                let slug = market.slug.clone().unwrap_or_default();
                
                // Pre-filter: only process markets for this asset with 2 outcomes
                let text = format!("{} {}", slug, market.question).to_lowercase();
                
                // Check if asset matches
                let asset_match = match asset {
                    Asset::BTC => text.contains("btc") || text.contains("bitcoin"),
                    Asset::ETH => text.contains("eth") || text.contains("ethereum"),
                    _ => false,
                };
                
                // Must have exactly 2 outcomes (binary markets)
                let is_binary = market.outcomes.len() == 2;
                
                if !asset_match || !is_binary {
                    debug!("⏭️ Skipping {}: asset_match={} is_binary={} (slug='{}')", 
                        condition_id, asset_match, is_binary, slug);
                    continue;
                }
                
                info!("🎯 Processing binary BTC/ETH market: {} (slug='{}' outcomes={:?})", 
                    condition_id, slug, market.outcomes);
                
                if let Some(discovered) = self.convert_market_response(market, asset) {
                    info!("✅ Converted market: {} (tf={:?}, end={})", 
                        discovered.condition_id, discovered.timeframe, discovered.end_date);
                    all_markets.push(discovered);
                } else {
                    info!("❌ Failed to convert: {} q='{}' slug='{}'", 
                        condition_id, q_preview, slug);
                }
            }
        }

        // Also search without tag - get active markets and filter
        info!("Fetching active markets (no tag) for {:?}", asset);
        let active_markets = self.rest_client
            .get_markets_page(&self.gamma_url, None::<&str>, 200, 0)
            .await?;
        
        info!("Active markets returned {} total", active_markets.len());
        
        for market in active_markets {
            let text = format!("{} {}", 
                market.slug.as_deref().unwrap_or(""), 
                market.question
            ).to_lowercase();
            
            // Filter for up/down BTC/ETH markets
            let is_updown = text.contains("up or down") || text.contains("updown");
            let asset_match = match asset {
                Asset::BTC => text.contains("btc") || text.contains("bitcoin"),
                Asset::ETH => text.contains("eth") || text.contains("ethereum"),
                _ => false,
            };
            
            if is_updown && asset_match {
                if let Some(discovered) = self.convert_market_response(market, asset) {
                    if !all_markets.iter().any(|m| m.condition_id == discovered.condition_id) {
                        info!("✅ Converted from active search: {} (tf={:?})", 
                            discovered.condition_id, discovered.timeframe);
                        all_markets.push(discovered);
                    }
                }
            }
        }

        info!("🎯 Total candidate markets for {:?}: {}", asset, all_markets.len());
        Ok(all_markets)
    }

    /// Convert MarketResponse to DiscoveredMarket
    fn convert_market_response(
        &self,
        market: MarketResponse,
        asset: Asset,
    ) -> Option<DiscoveredMarket> {
        let condition_id = &market.condition_id;
        
        // Parse end date
        let end_date = market.end_date.as_deref()
            .and_then(|d| {
                DateTime::parse_from_rfc3339(d)
                    .or_else(|_| DateTime::parse_from_str(d, "%Y-%m-%dT%H:%M:%S%.fZ"))
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .unwrap_or_else(|| Utc::now() + chrono::Duration::hours(1));

        // Parse outcome prices
        let outcome_prices: Vec<f64> = market.outcome_prices
            .iter()
            .filter_map(|p| p.parse::<f64>().ok())
            .collect();

        // Determine timeframe from question/slug/end_date
        let text = format!("{} {}", market.slug.as_deref().unwrap_or(""), market.question)
            .to_lowercase();
        
        // Calculate time until expiry
        let now = Utc::now();
        let duration_to_end = end_date.signed_duration_since(now);
        let minutes_to_end = duration_to_end.num_minutes();
        
        // Detect timeframe
        let timeframe = if text.contains("15m") || text.contains("15-minute") || text.contains("15 minute") {
            Timeframe::Min15
        } else if text.contains("1h") || text.contains("1-hour") || text.contains("hourly") {
            Timeframe::Hour1
        } else if text.contains("up or down") || text.contains("updown") || text.contains("up-down") {
            // For up/down markets, infer from end_date
            if minutes_to_end > 0 && minutes_to_end <= 20 {
                Timeframe::Min15
            } else if minutes_to_end > 20 && minutes_to_end <= 90 {
                Timeframe::Hour1
            } else if minutes_to_end > 90 && minutes_to_end <= 180 {
                // Could be 1h market with more time remaining
                Timeframe::Hour1
            } else {
                info!("Market {} rejected: up/down market but invalid expiry ({} min)", 
                    condition_id, minutes_to_end);
                return None;
            }
        } else if minutes_to_end > 0 && minutes_to_end <= 20 {
            Timeframe::Min15
        } else if minutes_to_end > 0 && minutes_to_end <= 90 {
            Timeframe::Hour1
        } else {
            info!("Market {} rejected: can't determine timeframe (text='{}' expires_in={}min)", 
                condition_id, text.chars().take(60).collect::<String>(), minutes_to_end);
            return None;
        };
        
        // Check token_ids
        if market.clob_token_ids.len() != 2 {
            info!("Market {} rejected: expected 2 token_ids, got {}", 
                condition_id, market.clob_token_ids.len());
            return None;
        }
        
        info!("Market {} CONVERTED: slug='{}' tf={:?} tokens={} outcomes={:?}",
            condition_id, market.slug.as_deref().unwrap_or(""), timeframe,
            market.clob_token_ids.len(), market.outcomes);

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
            volume: 0.0, // Not available in MarketResponse
            liquidity: market.liquidity_num.unwrap_or(0.0),
            spread: match (market.best_bid, market.best_ask) {
                (Some(bid), Some(ask)) if ask > bid => ask - bid,
                _ => 0.0,
            },
            asset,
            timeframe,
        })
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
            debug!("Market {} rejected: active={} closed={}", condition_id, market.active, market.closed);
            return false;
        }

        // Must not have ended
        if now > market.end_date {
            debug!("Market {} rejected: already ended (end={} now={})", condition_id, market.end_date, now);
            return false;
        }

        // Must have exactly 2 outcomes/tokens
        if market.token_ids.len() != 2 || market.outcomes.len() != 2 {
            debug!("Market {} rejected: token_ids={} outcomes={}", 
                condition_id, market.token_ids.len(), market.outcomes.len());
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
            debug!("Market {} rejected: asset mismatch (slug='{}' question='{}')", 
                condition_id, market.slug, market.question);
            return false;
        }

        // Verify timeframe match
        if market.timeframe != timeframe {
            debug!("Market {} rejected: timeframe mismatch (market={:?} requested={:?})", 
                condition_id, market.timeframe, timeframe);
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
            debug!("Market {} rejected: outcomes missing up={} down={} (outcomes={:?})", 
                condition_id, has_up, has_down, market.outcomes);
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
            debug!("Market {} rejected: ends too far in future ({} > {:?})", 
                condition_id, time_to_end, max_lookahead);
            return false;
        }

        info!("Market {} VALIDATED for {:?} {:?}", condition_id, asset, timeframe);
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
        self.tracked_markets
            .values()
            .any(|m| {
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
        matches!(self, MarketChange::Rollover { .. } | MarketChange::NewMarket { .. })
    }
}
