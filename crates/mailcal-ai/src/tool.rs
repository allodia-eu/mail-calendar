//! `emit_json`: the one tool, forced, through which a structured answer arrives.
//!
//! The tool's parameters are the JSON Schema of the type the answer is read into, derived from
//! that type, so what a model is shown and what is parsed cannot drift. A backend that answers in
//! text instead of calling the tool (some self-hosted servers ignore `tool_choice`) is read from
//! the first JSON object in its text; anything else is [`AiError::Malformed`], never a guess.

use schemars::{JsonSchema, SchemaGenerator, generate::SchemaSettings};
use serde::de::DeserializeOwned;

use crate::{
    AiError,
    wire::{AssistantMessage, ChatResponse, FunctionSpec, Tool, ToolChoice, ToolChoiceFunction},
};

/// The tool's name.
pub(crate) const NAME: &str = "emit_json";

/// The tool and the choice that forces it, for an answer of type `T`.
pub(crate) fn forced<T: JsonSchema>(description: &'static str) -> (Tool, ToolChoice) {
    // Inlined, because `$ref` support in function parameters is uneven across providers and
    // self-hosted servers; the schemas here are shallow.
    let settings = SchemaSettings::draft2020_12().with(|settings| {
        settings.inline_subschemas = true;
    });
    let mut parameters = SchemaGenerator::new(settings)
        .into_root_schema_for::<T>()
        .to_value();
    if let Some(object) = parameters.as_object_mut() {
        object.remove("$schema");
        object.remove("title");
    }
    (
        Tool {
            kind: "function",
            function: FunctionSpec {
                name: NAME,
                description,
                parameters,
            },
        },
        ToolChoice {
            kind: "function",
            function: ToolChoiceFunction { name: NAME },
        },
    )
}

/// The answer's `emit_json` arguments, read as `T`.
///
/// # Errors
///
/// Returns [`AiError::Malformed`] when there is no answer, or neither a call nor the text holds
/// a `T`.
pub(crate) fn read<T: DeserializeOwned>(response: &ChatResponse) -> Result<T, AiError> {
    let answer = response.answer().ok_or(AiError::Malformed)?;
    if let Some(call) = answer
        .tool_calls
        .iter()
        .find(|call| call.function.name == NAME)
    {
        return serde_json::from_str(&call.function.arguments).map_err(|_| AiError::Malformed);
    }
    from_text(answer)
}

fn from_text<T: DeserializeOwned>(answer: &AssistantMessage) -> Result<T, AiError> {
    let text = answer.content.as_deref().unwrap_or_default();
    let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) else {
        return Err(AiError::Malformed);
    };
    if end < start {
        return Err(AiError::Malformed);
    }
    serde_json::from_str(&text[start..=end]).map_err(|_| AiError::Malformed)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{forced, read};
    use crate::{
        AiError, LanguageStyle,
        test_support::{text_answer, tool_answer},
    };

    #[test]
    fn the_schema_is_self_contained_and_describes_the_type() {
        let (tool, choice) = forced::<LanguageStyle>("describe");
        let parameters = &tool.function.parameters;
        assert!(parameters.get("$schema").is_none());
        assert!(parameters.get("$defs").is_none());
        assert!(!parameters.to_string().contains("$ref"));
        assert_eq!(
            parameters["properties"]["greetings"]["items"]["properties"]["text"]["type"],
            "string"
        );
        // Fields a newer version wrote are kept on read-back, never asked of a model.
        assert!(parameters["properties"].get("unknown").is_none());
        assert_eq!(choice.function.name, "emit_json");
    }

    #[test]
    fn a_tool_call_is_read() {
        let style: LanguageStyle = read(&tool_answer(
            &json!({ "register": "je", "typical_words": 80 }),
        ))
        .unwrap();
        assert_eq!(style.register, "je");
        assert_eq!(style.typical_words, 80);
    }

    #[test]
    fn a_json_answer_in_text_is_read_when_no_tool_was_called() {
        let answer = text_answer("Here you go:\n```json\n{\"register\": \"u\"}\n```");
        let style: LanguageStyle = read(&answer).unwrap();
        assert_eq!(style.register, "u");
    }

    #[test]
    fn anything_else_is_malformed() {
        assert_eq!(
            read::<LanguageStyle>(&text_answer("I cannot help with that.")).unwrap_err(),
            AiError::Malformed
        );
        assert_eq!(
            read::<LanguageStyle>(&tool_answer(&json!({ "greetings": "Hi" }))).unwrap_err(),
            AiError::Malformed
        );
    }
}
