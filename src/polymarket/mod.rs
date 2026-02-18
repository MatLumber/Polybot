//! Polymarket-native strategy and execution helpers.

pub mod edge_engine;
pub mod execution_engine;
pub mod fee_engine;
pub mod kelly;
pub mod labeling;
pub mod market_state;

pub use edge_engine::{estimate_expected_value, ExpectedValueQuote, SlippageQuote};
pub use execution_engine::{plan_buy_execution, ExecutionPlan};
pub use fee_engine::{estimate_roundtrip_fees, fee_rate_from_price, FeeQuote};
pub use kelly::{compute_fractional_kelly, KellyQuote};
pub use labeling::{label_from_resolution, ResolvedLabel};
pub use market_state::{
    ExecutionPolicy, MarketStateStore, MarketWindowKey, OrderIntent, Outcome, OutcomeToken,
    PositionLifecycleState, ResolvedOutcome, TokenRoute,
};
