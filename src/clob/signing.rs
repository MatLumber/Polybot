//! EIP-712 Signing for Polymarket CLOB
//!
//! Implements gasless order signing as per Polymarket specification.

use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use chrono::Utc;
use ethers::signers::{Signer, Wallet};
use ethers::types::transaction::eip712::{EIP712Domain, Eip712DomainType, TypedData, Types};
use ethers::types::{Address, U256};
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde_json::Value;
use sha2::Sha256;

use super::types::{Order, Side};

const POLYMARKET_CTF_EXCHANGE_DOMAIN: &str = "Polymarket CTF Exchange";
const CLOB_AUTH_DOMAIN: &str = "ClobAuthDomain";
const DOMAIN_VERSION: &str = "1";
const CLOB_AUTH_MESSAGE: &str = "This message attests that I control the given wallet";

const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

/// CTF Exchange contract address on Polygon
pub fn ctf_exchange_address() -> Address {
    // Official Polygon exchange contract from Polymarket client config.
    "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"
        .parse()
        .expect("valid Polygon exchange address")
}

fn ctf_exchange_address_for_chain(chain_id: u64) -> Result<Address> {
    match chain_id {
        137 => Ok(ctf_exchange_address()),
        80002 => Ok("0xdFE02Eb6733538f8Ea35D585af8DE5958AD99E40"
            .parse()
            .context("invalid Amoy exchange address constant")?),
        _ => bail!("Unsupported chain_id {} for CLOB order signing", chain_id),
    }
}

fn order_amounts(order: &Order) -> (U256, U256) {
    // Order.size is in shares for CLOB payload construction.
    // BUY -> makerAmount: USDC notional, takerAmount: shares.
    // SELL -> makerAmount: shares, takerAmount: USDC notional.
    let shares_scaled = (order.size.max(0.0) * 1_000_000.0).round() as u128;
    let usdc_scaled = (order.size.max(0.0) * order.price.max(0.0) * 1_000_000.0).round() as u128;
    match order.side {
        Side::Buy => (U256::from(usdc_scaled), U256::from(shares_scaled)),
        Side::Sell => (U256::from(shares_scaled), U256::from(usdc_scaled)),
    }
}

fn order_typed_data(order: &Order, maker: Address, chain_id: u64) -> Result<TypedData> {
    let token_id = U256::from_dec_str(&order.token_id)
        .with_context(|| format!("Invalid token_id '{}' for order signing", order.token_id))?;
    let (maker_amount, taker_amount) = order_amounts(order);

    let domain = EIP712Domain {
        name: Some(POLYMARKET_CTF_EXCHANGE_DOMAIN.to_string()),
        version: Some(DOMAIN_VERSION.to_string()),
        chain_id: Some(chain_id.into()),
        verifying_contract: Some(ctf_exchange_address_for_chain(chain_id)?),
        salt: None,
    };

    let mut types: Types = BTreeMap::new();
    types.insert(
        "Order".to_string(),
        vec![
            Eip712DomainType {
                name: "salt".to_string(),
                r#type: "uint256".to_string(),
            },
            Eip712DomainType {
                name: "maker".to_string(),
                r#type: "address".to_string(),
            },
            Eip712DomainType {
                name: "signer".to_string(),
                r#type: "address".to_string(),
            },
            Eip712DomainType {
                name: "taker".to_string(),
                r#type: "address".to_string(),
            },
            Eip712DomainType {
                name: "tokenId".to_string(),
                r#type: "uint256".to_string(),
            },
            Eip712DomainType {
                name: "makerAmount".to_string(),
                r#type: "uint256".to_string(),
            },
            Eip712DomainType {
                name: "takerAmount".to_string(),
                r#type: "uint256".to_string(),
            },
            Eip712DomainType {
                name: "expiration".to_string(),
                r#type: "uint256".to_string(),
            },
            Eip712DomainType {
                name: "nonce".to_string(),
                r#type: "uint256".to_string(),
            },
            Eip712DomainType {
                name: "feeRateBps".to_string(),
                r#type: "uint256".to_string(),
            },
            Eip712DomainType {
                name: "side".to_string(),
                r#type: "uint8".to_string(),
            },
            Eip712DomainType {
                name: "signatureType".to_string(),
                r#type: "uint8".to_string(),
            },
        ],
    );

    let mut message = BTreeMap::<String, Value>::new();
    message.insert("salt".to_string(), Value::String(order.salt.to_string()));
    message.insert("maker".to_string(), Value::String(format!("{:#x}", maker)));
    message.insert("signer".to_string(), Value::String(format!("{:#x}", maker)));
    message.insert("taker".to_string(), Value::String(ZERO_ADDRESS.to_string()));
    message.insert("tokenId".to_string(), Value::String(token_id.to_string()));
    message.insert(
        "makerAmount".to_string(),
        Value::String(maker_amount.to_string()),
    );
    message.insert(
        "takerAmount".to_string(),
        Value::String(taker_amount.to_string()),
    );
    message.insert(
        "expiration".to_string(),
        Value::String(order.expiration.to_string()),
    );
    message.insert("nonce".to_string(), Value::String(order.nonce.to_string()));
    message.insert("feeRateBps".to_string(), Value::String("0".to_string()));
    message.insert(
        "side".to_string(),
        Value::from(match order.side {
            Side::Buy => 0_u8,
            Side::Sell => 1_u8,
        }),
    );
    message.insert(
        "signatureType".to_string(),
        Value::from(order.signature_type),
    );

    Ok(TypedData {
        domain,
        types,
        primary_type: "Order".to_string(),
        message,
    })
}

/// Sign an order using EIP-712
pub async fn sign_order(order: &mut Order, private_key: &str, chain_id: u64) -> Result<()> {
    let wallet: Wallet<_> = private_key
        .parse()
        .context("Invalid private key for EIP-712 order signing")?;
    let maker = order.maker.unwrap_or_else(|| wallet.address());
    order.maker = Some(maker);

    if order.nonce == 0 {
        order.nonce = rand::random::<u64>();
    }
    if order.salt.is_zero() {
        order.salt = U256::from(rand::random::<u64>());
    }

    let typed = order_typed_data(order, maker, chain_id)?;
    let signature = wallet
        .sign_typed_data(&typed)
        .await
        .context("Failed to sign order typed data")?;
    let sig = signature.to_string();
    order.signature = Some(if sig.starts_with("0x") {
        sig
    } else {
        format!("0x{}", sig)
    });

    Ok(())
}

/// Derive API key and secret from private key
pub async fn derive_api_credentials(private_key: &str) -> Result<(String, String, Address)> {
    let wallet: Wallet<_> = private_key.parse().context("Invalid private key")?;
    let address = wallet.address();
    let chain_id = std::env::var("POLY_CHAIN_ID")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(137);
    let base_url = std::env::var("POLY_CLOB_URL")
        .unwrap_or_else(|_| "https://clob.polymarket.com".to_string())
        .trim_end_matches('/')
        .to_string();

    let timestamp = Utc::now().timestamp();
    let nonce = rand::random::<u64>();
    let l1_signature = create_l1_signature(private_key, chain_id, timestamp, nonce).await?;

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        "POLY_ADDRESS",
        HeaderValue::from_str(&format!("{:#x}", address))
            .context("Invalid POLY_ADDRESS header value")?,
    );
    headers.insert(
        "POLY_SIGNATURE",
        HeaderValue::from_str(&l1_signature).context("Invalid POLY_SIGNATURE header value")?,
    );
    headers.insert(
        "POLY_TIMESTAMP",
        HeaderValue::from_str(&timestamp.to_string())
            .context("Invalid POLY_TIMESTAMP header value")?,
    );
    headers.insert(
        "POLY_NONCE",
        HeaderValue::from_str(&nonce.to_string()).context("Invalid POLY_NONCE header value")?,
    );

    let client = reqwest::Client::new();
    let create_url = format!("{}/auth/api-key", base_url);
    let create_resp = client
        .post(&create_url)
        .headers(headers.clone())
        .body("{}")
        .send()
        .await
        .context("Failed POST /auth/api-key")?;

    let raw = if create_resp.status().is_success() {
        create_resp
            .json::<Value>()
            .await
            .context("Failed parsing /auth/api-key response")?
    } else {
        let derive_url = format!("{}/auth/derive-api-key", base_url);
        let derive_resp = client
            .get(&derive_url)
            .headers(headers)
            .send()
            .await
            .context("Failed GET /auth/derive-api-key")?;
        if !derive_resp.status().is_success() {
            let create_status = create_resp.status();
            let create_body = create_resp.text().await.unwrap_or_default();
            let derive_status = derive_resp.status();
            let derive_body = derive_resp.text().await.unwrap_or_default();
            bail!(
                "L1 auth endpoints failed. create: {} [{}], derive: {} [{}]",
                create_status,
                create_body,
                derive_status,
                derive_body
            );
        }
        derive_resp
            .json::<Value>()
            .await
            .context("Failed parsing /auth/derive-api-key response")?
    };

    fn pick(value: &Value, keys: &[&str]) -> Option<String> {
        for key in keys {
            if let Some(s) = value.get(*key).and_then(|v| v.as_str()) {
                if !s.trim().is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        None
    }

    let data = raw.get("data").unwrap_or(&raw);
    let api_key = pick(data, &["apiKey", "api_key", "key", "id"])
        .context("Missing api key in auth response")?;
    let api_secret = pick(data, &["secret", "apiSecret", "api_secret"])
        .context("Missing api secret in auth response")?;
    if let Some(passphrase) = pick(data, &["passphrase", "apiPassphrase", "api_passphrase"]) {
        std::env::set_var("POLY_API_PASSPHRASE", passphrase);
    }

    Ok((api_key, api_secret, address))
}

/// Create L1 signature for `/auth/*` endpoints.
pub async fn create_l1_signature(
    private_key: &str,
    chain_id: u64,
    timestamp: i64,
    nonce: u64,
) -> Result<String> {
    let wallet: Wallet<_> = private_key
        .parse()
        .context("Invalid private key for L1 signature")?;

    let domain = EIP712Domain {
        name: Some(CLOB_AUTH_DOMAIN.to_string()),
        version: Some(DOMAIN_VERSION.to_string()),
        chain_id: Some(chain_id.into()),
        verifying_contract: None,
        salt: None,
    };

    let mut types: Types = BTreeMap::new();
    types.insert(
        "ClobAuth".to_string(),
        vec![
            Eip712DomainType {
                name: "address".to_string(),
                r#type: "address".to_string(),
            },
            Eip712DomainType {
                name: "timestamp".to_string(),
                r#type: "string".to_string(),
            },
            Eip712DomainType {
                name: "nonce".to_string(),
                r#type: "uint256".to_string(),
            },
            Eip712DomainType {
                name: "message".to_string(),
                r#type: "string".to_string(),
            },
        ],
    );

    let mut message = BTreeMap::<String, Value>::new();
    message.insert(
        "address".to_string(),
        Value::String(format!("{:#x}", wallet.address())),
    );
    message.insert(
        "timestamp".to_string(),
        Value::String(timestamp.to_string()),
    );
    message.insert("nonce".to_string(), Value::String(nonce.to_string()));
    message.insert(
        "message".to_string(),
        Value::String(CLOB_AUTH_MESSAGE.to_string()),
    );

    let typed = TypedData {
        domain,
        types,
        primary_type: "ClobAuth".to_string(),
        message,
    };
    let sig = wallet
        .sign_typed_data(&typed)
        .await
        .context("Failed to sign L1 auth typed data")?;
    let sig = sig.to_string();
    Ok(if sig.starts_with("0x") {
        sig
    } else {
        format!("0x{}", sig)
    })
}

/// Create L2 HMAC signature for authenticated CLOB REST requests.
pub fn create_l2_signature(
    api_secret: &str,
    timestamp: i64,
    method: &str,
    request_path: &str,
    body: Option<&str>,
) -> Result<String> {
    let secret_bytes = general_purpose::URL_SAFE_NO_PAD
        .decode(api_secret)
        .or_else(|_| general_purpose::URL_SAFE.decode(api_secret))
        .context("Failed decoding POLY_API_SECRET as url-safe base64")?;

    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(&secret_bytes).context("Failed to initialize HMAC")?;
    let payload = format!(
        "{}{}{}{}",
        timestamp,
        method.to_uppercase(),
        request_path,
        body.unwrap_or("")
    );
    mac.update(payload.as_bytes());
    Ok(general_purpose::URL_SAFE.encode(mac.finalize().into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sign_order_sets_hex_signature() {
        let mut order = Order::new("1".to_string(), Side::Buy, 0.50, 10.0);
        let pk = "0x59c6995e998f97a5a0044966f0945387dc9f5a59e86cdc84e64546a1d8f76d59";
        sign_order(&mut order, pk, 137).await.unwrap();
        let sig = order.signature.unwrap();
        assert!(sig.starts_with("0x"));
        assert!(sig.len() >= 130);
    }
}
