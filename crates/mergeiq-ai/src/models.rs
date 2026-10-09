//! What each model accepts: effort, server-side fallbacks, structured output.

use crate::provider::Effort;

/// Capabilities of an Anthropic model, by id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnthropicCaps {
    /// Accepts `output_config.effort`.
    pub effort: bool,
    /// Highest effort level the model accepts.
    pub max_effort: Effort,
    /// Accepts `thinking: {type: "adaptive"}`.
    pub adaptive_thinking: bool,
    /// Accepts the server-side `fallbacks` parameter (with its beta header).
    pub fallbacks: bool,
    /// Supports `output_config.format` with a JSON schema.
    pub structured_output: bool,
}

/// Looks up what `model` accepts. Unknown ids are treated like a current model without
/// fallbacks, which is the safe superset.
pub fn anthropic_caps(model: &str) -> AnthropicCaps {
    let m = model;
    let is = |prefix: &str| m.starts_with(prefix);
    // Models that take the server-side `fallbacks: "default"` parameter.
    let fallbacks = is("claude-fable-")
        || is("claude-mythos-")
        || m == "claude-opus-5"
        || is("claude-opus-5-")
        || is("claude-sonnet-5-5");
    if is("claude-haiku-4")
        || is("claude-3")
        || is("claude-sonnet-3")
        || is("claude-opus-4-1")
        || is("claude-opus-4-0")
    {
        // Pre-4.6 models: no effort, no adaptive thinking (thinking stays off by omission).
        return AnthropicCaps {
            effort: false,
            max_effort: Effort::High,
            adaptive_thinking: false,
            fallbacks: false,
            structured_output: is("claude-haiku-4") || is("claude-opus-4-1"),
        };
    }
    if is("claude-opus-4-5") {
        return AnthropicCaps {
            effort: true,
            max_effort: Effort::High,
            adaptive_thinking: false,
            fallbacks: false,
            structured_output: true,
        };
    }
    AnthropicCaps {
        effort: true,
        max_effort: Effort::Max,
        adaptive_thinking: true,
        fallbacks,
        structured_output: true,
    }
}

/// The effort level to send: `wanted`, adjusted to what `model` accepts.
pub fn clamp_effort(model: &str, wanted: Effort) -> Option<Effort> {
    let caps = anthropic_caps(model);
    if !caps.effort {
        return None;
    }
    let no_xhigh = model.starts_with("claude-opus-4-6")
        || model.starts_with("claude-sonnet-4-6")
        || model.starts_with("claude-opus-4-5");
    Some(match wanted {
        Effort::Xhigh if no_xhigh => Effort::High,
        Effort::Max if caps.max_effort != Effort::Max => caps.max_effort,
        other if rank(other) > rank(caps.max_effort) => caps.max_effort,
        other => other,
    })
}

fn rank(e: Effort) -> u8 {
    match e {
        Effort::Low => 0,
        Effort::Medium => 1,
        Effort::High => 2,
        Effort::Xhigh => 3,
        Effort::Max => 4,
    }
}

/// Whether an OpenAI-family model supports `response_format: json_schema` (strict).
pub fn openai_json_schema(model: &str) -> bool {
    ["gpt-4o", "gpt-4.1", "gpt-4.5", "gpt-5", "o1", "o3", "o4"]
        .iter()
        .any(|p| model.starts_with(p) || model.contains(&format!("/{p}")))
}

/// Whether an OpenAI-family model takes `reasoning_effort` (the reasoning models).
pub fn openai_takes_reasoning_effort(model: &str) -> bool {
    let bare = model.rsplit('/').next().unwrap_or(model);
    ["gpt-5", "o1", "o3", "o4"]
        .iter()
        .any(|p| bare.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_models_take_effort_adaptive_thinking_and_structured_output() {
        for m in [
            "claude-opus-5",
            "claude-opus-5-5",
            "claude-sonnet-5",
            "claude-sonnet-5-5",
            "claude-haiku-5-5",
            "claude-fable-5-1",
        ] {
            let c = anthropic_caps(m);
            assert!(
                c.effort && c.adaptive_thinking && c.structured_output,
                "{m}"
            );
        }
    }

    #[test]
    fn fallbacks_only_where_the_api_accepts_them() {
        for m in [
            "claude-opus-5",
            "claude-opus-5-5",
            "claude-fable-5-1",
            "claude-sonnet-5-5",
        ] {
            assert!(anthropic_caps(m).fallbacks, "{m}");
        }
        for m in [
            "claude-sonnet-5",
            "claude-opus-4-8",
            "claude-haiku-5-5",
            "claude-haiku-4-5",
            "claude-sonnet-4-6",
        ] {
            assert!(!anthropic_caps(m).fallbacks, "{m}");
        }
    }

    #[test]
    fn old_models_get_no_effort_and_no_adaptive_thinking() {
        let c = anthropic_caps("claude-haiku-4-5");
        assert!(!c.effort && !c.adaptive_thinking);
        assert_eq!(clamp_effort("claude-haiku-4-5", Effort::High), None);
        assert!(!anthropic_caps("claude-3-5-sonnet-latest").adaptive_thinking);
    }

    #[test]
    fn effort_is_clamped_to_what_the_model_accepts() {
        assert_eq!(
            clamp_effort("claude-opus-5", Effort::Xhigh),
            Some(Effort::Xhigh)
        );
        assert_eq!(
            clamp_effort("claude-opus-5", Effort::Max),
            Some(Effort::Max)
        );
        assert_eq!(
            clamp_effort("claude-sonnet-4-6", Effort::Xhigh),
            Some(Effort::High)
        );
        assert_eq!(
            clamp_effort("claude-sonnet-4-6", Effort::Max),
            Some(Effort::Max)
        );
        assert_eq!(
            clamp_effort("claude-opus-4-5", Effort::Max),
            Some(Effort::High)
        );
        assert_eq!(
            clamp_effort("claude-opus-5", Effort::Low),
            Some(Effort::Low)
        );
    }

    #[test]
    fn openai_capabilities() {
        assert!(openai_json_schema("gpt-4.1"));
        assert!(openai_json_schema("gpt-4o-mini"));
        assert!(openai_json_schema("openai/gpt-4.1"));
        assert!(!openai_json_schema("gpt-3.5-turbo"));
        assert!(!openai_json_schema("some-local-model"));
        assert!(openai_takes_reasoning_effort("gpt-5"));
        assert!(openai_takes_reasoning_effort("openai/o4-mini"));
        assert!(!openai_takes_reasoning_effort("gpt-4.1"));
    }
}
