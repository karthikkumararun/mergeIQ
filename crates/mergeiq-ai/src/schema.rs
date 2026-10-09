//! The structured-output schema for suggestions, and strict validation of responses.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// How sure the model is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    /// Confident.
    High,
    /// Reasonably sure.
    Medium,
    /// A guess.
    Low,
}

/// How the resolution relates to the two sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    /// The left side as is.
    Left,
    /// The right side as is.
    Right,
    /// Both sides' changes, concatenated or interleaved.
    Both,
    /// Both sides' intent merged into new code.
    Combined,
    /// Something else.
    New,
}

/// A suggested resolution for one conflict chunk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct Suggestion {
    /// The text that replaces the chunk's result range.
    pub resolution: String,
    /// A short explanation of the choice.
    pub explanation: String,
    /// The model's confidence.
    pub confidence: Confidence,
    /// How the resolution was derived.
    pub strategy: Strategy,
    /// Things the user should double-check.
    pub risks: Vec<String>,
}

/// The JSON schema (`additionalProperties: false`, every field required).
pub fn suggestion_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "resolution": {
                "type": "string",
                "description": "The exact text that replaces the conflicting region, with line breaks, and no conflict markers."
            },
            "explanation": {
                "type": "string",
                "description": "Two or three sentences on what each side did and how the resolution combines them."
            },
            "confidence": { "type": "string", "enum": ["high", "medium", "low"] },
            "strategy": { "type": "string", "enum": ["left", "right", "both", "combined", "new"] },
            "risks": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Short notes on what could be wrong or needs checking. Empty if none."
            }
        },
        "required": ["resolution", "explanation", "confidence", "strategy", "risks"],
        "additionalProperties": false
    })
}

/// Validates `text` (a JSON document) against the schema and parses it.
pub fn parse_suggestion(text: &str) -> Result<Suggestion, String> {
    let value: Value =
        serde_json::from_str(text.trim()).map_err(|e| format!("not valid JSON: {e}"))?;
    let schema = suggestion_schema();
    let validator = jsonschema::validator_for(&schema).map_err(|e| e.to_string())?;
    let errors: Vec<String> = validator
        .iter_errors(&value)
        .map(|e| format!("{e} at {}", e.instance_path()))
        .collect();
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    serde_json::from_value(value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> Value {
        json!({
            "resolution": "fn a() {}\n",
            "explanation": "Both added functions.",
            "confidence": "high",
            "strategy": "both",
            "risks": []
        })
    }

    #[test]
    fn a_valid_payload_parses() {
        let s = parse_suggestion(&valid().to_string()).unwrap();
        assert_eq!(s.confidence, Confidence::High);
        assert_eq!(s.strategy, Strategy::Both);
        assert_eq!(s.resolution, "fn a() {}\n");
        assert!(s.risks.is_empty());
    }

    #[test]
    fn surrounding_whitespace_is_tolerated() {
        assert!(parse_suggestion(&format!("\n  {}  \n", valid())).is_ok());
    }

    #[test]
    fn invalid_payloads_are_rejected_with_a_reason() {
        type Mutation = Box<dyn Fn(&mut Value)>;
        let cases: Vec<(&str, Mutation)> = vec![
            (
                "missing field",
                Box::new(|v| {
                    v.as_object_mut().unwrap().remove("risks");
                }),
            ),
            (
                "bad confidence",
                Box::new(|v| v["confidence"] = json!("certain")),
            ),
            ("bad strategy", Box::new(|v| v["strategy"] = json!("merge"))),
            ("wrong type", Box::new(|v| v["resolution"] = json!(5))),
            ("extra property", Box::new(|v| v["notes"] = json!("x"))),
            ("risks not strings", Box::new(|v| v["risks"] = json!([1]))),
        ];
        for (name, mutate) in cases {
            let mut v = valid();
            mutate(&mut v);
            let err = parse_suggestion(&v.to_string()).unwrap_err();
            assert!(!err.is_empty(), "{name}");
        }
        assert!(parse_suggestion("not json")
            .unwrap_err()
            .contains("not valid JSON"));
        assert!(parse_suggestion("[]").is_err());
        assert!(parse_suggestion("").is_err());
    }

    #[test]
    fn the_schema_is_strict() {
        let s = suggestion_schema();
        assert_eq!(s["additionalProperties"], json!(false));
        let required: Vec<&str> = s["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        let props: Vec<&String> = s["properties"].as_object().unwrap().keys().collect();
        assert_eq!(required.len(), props.len());
        for p in props {
            assert!(required.contains(&p.as_str()), "{p} must be required");
        }
    }
}
