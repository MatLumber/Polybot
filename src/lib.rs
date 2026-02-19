//! PolyBot Library
//!
//! ML-powered trading system for Polymarket

pub mod backtesting;
pub mod clob;
pub mod config;
pub mod features;
pub mod ml_engine;
pub mod oracle;
pub mod paper_trading;
pub mod persistence;
pub mod polymarket;
pub mod risk;
pub mod strategy;
pub mod types;

#[cfg(feature = "dashboard")]
pub mod dashboard;
