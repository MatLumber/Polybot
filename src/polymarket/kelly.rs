#[derive(Debug, Clone, Copy)]
pub struct KellyQuote {
    pub p_adj: f64,
    pub b_eff: f64,
    pub f_raw: f64,
    pub f_capped: f64,
    pub f_fractional: f64,
}

pub fn compute_fractional_kelly(
    p_model: f64,
    sigma_model: f64,
    share_price: f64,
    fractional: f64,
    cap: f64,
) -> KellyQuote {
    let penalty_uncertainty = 1.28 * sigma_model.max(0.0);
    let p_adj = (p_model - penalty_uncertainty).clamp(0.01, 0.99);

    let price = share_price.clamp(0.01, 0.99);
    let net_win = 1.0 - price;
    let net_loss = price;
    let b_eff = if net_loss > 0.0 {
        net_win / net_loss
    } else {
        0.0
    };

    let f_raw = if b_eff > 0.0 {
        ((b_eff * p_adj) - (1.0 - p_adj)) / b_eff
    } else {
        0.0
    };
    let f_capped = f_raw.max(0.0).min(cap.max(0.0));
    let f_fractional = f_capped * fractional.max(0.0);

    KellyQuote {
        p_adj,
        b_eff,
        f_raw,
        f_capped,
        f_fractional,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kelly_zero_when_negative_edge() {
        let q = compute_fractional_kelly(0.45, 0.01, 0.50, 0.25, 0.01);
        assert!(q.f_fractional <= 0.0000001);
    }
}
