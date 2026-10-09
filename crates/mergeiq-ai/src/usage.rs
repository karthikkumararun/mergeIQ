//! Token accounting per request and per session, with an editable price table.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::provider::ProviderKind;

/// Token usage and latency of one request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    /// Input tokens that were not served from the cache.
    pub input_tokens: u32,
    /// Output tokens.
    pub output_tokens: u32,
    /// Input tokens read from the prompt cache.
    pub cache_read_tokens: u32,
    /// Input tokens written to the prompt cache.
    pub cache_creation_tokens: u32,
    /// Wall-clock time in milliseconds.
    pub latency_ms: u32,
}

impl Usage {
    /// All input tokens, cached or not.
    pub fn total_input(&self) -> u32 {
        self.input_tokens
            .saturating_add(self.cache_read_tokens)
            .saturating_add(self.cache_creation_tokens)
    }

    /// Adds another request's tokens (latency is summed too).
    pub fn add(&mut self, other: &Usage) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.cache_read_tokens = self
            .cache_read_tokens
            .saturating_add(other.cache_read_tokens);
        self.cache_creation_tokens = self
            .cache_creation_tokens
            .saturating_add(other.cache_creation_tokens);
        self.latency_ms = self.latency_ms.saturating_add(other.latency_ms);
    }
}

/// US dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Price {
    /// Uncached input.
    pub input: f64,
    /// Output.
    pub output: f64,
    /// Cache reads.
    pub cache_read: f64,
    /// Cache writes.
    pub cache_write: f64,
}

impl Price {
    /// Estimated cost in dollars.
    pub fn cost(&self, usage: &Usage) -> f64 {
        let per = |tokens: u32, price: f64| f64::from(tokens) / 1_000_000.0 * price;
        per(usage.input_tokens, self.input)
            + per(usage.output_tokens, self.output)
            + per(usage.cache_read_tokens, self.cache_read)
            + per(usage.cache_creation_tokens, self.cache_write)
    }
}

/// Prices by model id. Lookups use the longest entry that is a prefix of the model id, so
/// dated or suffixed ids still match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PriceTable {
    /// Model id (or prefix) to price.
    pub entries: BTreeMap<String, Price>,
}

impl Default for PriceTable {
    /// Anthropic list prices (cache reads 10% and writes 125% of input, unless noted).
    fn default() -> Self {
        let p = |input: f64, output: f64, read: f64| Price {
            input,
            output,
            cache_read: read,
            cache_write: input * 1.25,
        };
        let entries = [
            ("claude-opus-5-5", p(4.0, 20.0, 0.20)),
            ("claude-opus-5", p(5.0, 25.0, 0.50)),
            ("claude-opus-4", p(5.0, 25.0, 0.50)),
            ("claude-sonnet-5", p(2.0, 10.0, 0.20)),
            ("claude-sonnet-4", p(3.0, 15.0, 0.30)),
            ("claude-haiku-5", p(0.10, 0.50, 0.01)),
            ("claude-haiku-4", p(1.0, 5.0, 0.10)),
            ("claude-fable-5", p(10.0, 50.0, 0.25)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        PriceTable { entries }
    }
}

impl PriceTable {
    /// The price for `model`, if the table knows it.
    pub fn price_for(&self, model: &str) -> Option<&Price> {
        self.entries
            .iter()
            .filter(|(k, _)| model.starts_with(k.as_str()))
            .max_by_key(|(k, _)| k.len())
            .map(|(_, v)| v)
    }

    /// Estimated cost in dollars, or `None` without a price entry.
    pub fn estimate(&self, model: &str, usage: &Usage) -> Option<f64> {
        self.price_for(model).map(|p| p.cost(usage))
    }
}

/// One finished request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UsageRecord {
    /// The provider.
    pub provider: ProviderKind,
    /// The model.
    pub model: String,
    /// What it used.
    pub usage: Usage,
    /// Estimated dollars, when a price is known.
    pub cost: Option<f64>,
}

/// Usage since the app started.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SessionUsage {
    /// Number of requests.
    pub requests: u32,
    /// Summed tokens.
    pub totals: Usage,
    /// Summed estimated cost over the requests that had a price.
    pub cost: Option<f64>,
    /// Every request, oldest first.
    pub records: Vec<UsageRecord>,
}

impl SessionUsage {
    /// Records a finished request.
    pub fn record(
        &mut self,
        provider: ProviderKind,
        model: &str,
        usage: Usage,
        prices: &PriceTable,
    ) {
        let cost = prices.estimate(model, &usage);
        self.requests += 1;
        self.totals.add(&usage);
        if let Some(c) = cost {
            self.cost = Some(self.cost.unwrap_or(0.0) + c);
        }
        self.records.push(UsageRecord {
            provider,
            model: model.to_string(),
            usage,
            cost,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(input: u32, output: u32, read: u32) -> Usage {
        Usage {
            input_tokens: input,
            output_tokens: output,
            cache_read_tokens: read,
            cache_creation_tokens: 0,
            latency_ms: 100,
        }
    }

    #[test]
    fn total_input_includes_cached_tokens() {
        let mut x = u(1000, 10, 4800);
        x.cache_creation_tokens = 200;
        assert_eq!(x.total_input(), 6000);
    }

    #[test]
    fn session_totals_are_the_sum_of_requests() {
        let prices = PriceTable::default();
        let mut s = SessionUsage::default();
        for _ in 0..3 {
            s.record(
                ProviderKind::Anthropic,
                "claude-opus-5",
                u(2000, 400, 4000),
                &prices,
            );
        }
        assert_eq!(s.requests, 3);
        assert_eq!(s.totals.input_tokens, 6000);
        assert_eq!(s.totals.output_tokens, 1200);
        assert_eq!(s.totals.cache_read_tokens, 12000);
        assert_eq!(s.records.len(), 3);
        // 2000 in * $5 + 400 out * $25 + 4000 cached * $0.50, per million, three times.
        let one = (2000.0 * 5.0 + 400.0 * 25.0 + 4000.0 * 0.5) / 1_000_000.0;
        assert!((s.cost.unwrap() - 3.0 * one).abs() < 1e-9);
    }

    #[test]
    fn longest_prefix_wins_and_unknown_models_have_no_estimate() {
        let t = PriceTable::default();
        assert_eq!(t.price_for("claude-opus-5-5").unwrap().input, 4.0);
        assert_eq!(t.price_for("claude-opus-5").unwrap().input, 5.0);
        assert_eq!(t.price_for("claude-opus-5-20260101").unwrap().input, 5.0);
        assert_eq!(t.price_for("claude-sonnet-4-6").unwrap().input, 3.0);
        assert!(t.price_for("gpt-4.1").is_none());
        assert!(t.estimate("gpt-4.1", &u(10, 10, 0)).is_none());
        let mut s = SessionUsage::default();
        s.record(ProviderKind::Openai, "gpt-4.1", u(10, 10, 0), &t);
        assert_eq!(s.cost, None);
        assert_eq!(s.requests, 1);
    }

    #[test]
    fn the_price_table_is_editable() {
        let mut t = PriceTable::default();
        t.entries.insert(
            "gpt-4.1".into(),
            Price {
                input: 2.0,
                output: 8.0,
                cache_read: 0.5,
                cache_write: 2.0,
            },
        );
        let cost = t.estimate("gpt-4.1", &u(1_000_000, 1_000_000, 0)).unwrap();
        assert!((cost - 10.0).abs() < 1e-9);
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(serde_json::from_str::<PriceTable>(&json).unwrap(), t);
    }
}
