/// Defines a specific prompting strategy or "kit".
pub trait PromptKit: Send + Sync {
    /// Unique identifier for the kit.
    fn id(&self) -> &str;

    /// User-friendly display name.
    fn name(&self) -> &str;

    /// Description for the TUI.
    fn description(&self) -> &str;

    /// The system prompt to set the persona.
    fn system_prompt(&self) -> String;

    /// Formats the user message.
    fn format_user_prompt(&self, transcript: &str, user_instruction: &str) -> String;
}

// -----------------------------------------------------------------------------
// 1. DraftReplyKit (Default)
// -----------------------------------------------------------------------------

pub struct DraftReplyKit;

impl PromptKit for DraftReplyKit {
    fn id(&self) -> &str {
        "reply"
    }

    fn name(&self) -> &str {
        "Draft Reply"
    }

    fn description(&self) -> &str {
        "Propose a draft reply based on the conversation."
    }

    fn system_prompt(&self) -> String {
        "You are a helpful assistant for a Telegram TUI client. Your goal is to draft a reply to the conversation provided. Be concise and conversational.".to_string()
    }

    fn format_user_prompt(&self, transcript: &str, user_instruction: &str) -> String {
        format!(
            "Context:\n{}\n\nInstruction: {}",
            transcript, user_instruction
        )
    }
}

// -----------------------------------------------------------------------------
// 2. SummarizeKit
// -----------------------------------------------------------------------------

pub struct SummarizeKit;

impl PromptKit for SummarizeKit {
    fn id(&self) -> &str {
        "summarize"
    }

    fn name(&self) -> &str {
        "Summarize"
    }

    fn description(&self) -> &str {
        "Summarize the conversation into key points."
    }

    fn system_prompt(&self) -> String {
        "You are an expert summarizer. Your goal is to condense the provided conversation into a clear, bulleted summary of key points and decisions.".to_string()
    }

    fn format_user_prompt(&self, transcript: &str, _user_instruction: &str) -> String {
        // We ignore user_instruction for pure summarization, or we could append it if needed.
        // For now, let's append it if present, in case user adds "focus on X".
        if _user_instruction.trim().is_empty() {
            format!("Conversation:\n{}", transcript)
        } else {
            format!(
                "Conversation:\n{}\n\nFocus on: {}",
                transcript, _user_instruction
            )
        }
    }
}

// -----------------------------------------------------------------------------
// 3. ActionItemsKit
// -----------------------------------------------------------------------------

pub struct ActionItemsKit;

impl PromptKit for ActionItemsKit {
    fn id(&self) -> &str {
        "action-items"
    }

    fn name(&self) -> &str {
        "Action Items"
    }

    fn description(&self) -> &str {
        "Extract tasks, deadliness, and action items."
    }

    fn system_prompt(&self) -> String {
        "You are a project manager assistant. Extract all action items, tasks, deadlines, and assignments from the conversation. Format as a checkbox list.".to_string()
    }

    fn format_user_prompt(&self, transcript: &str, _user_instruction: &str) -> String {
        if _user_instruction.trim().is_empty() {
            format!("Conversation:\n{}", transcript)
        } else {
            format!(
                "Conversation:\n{}\n\nAdditional Instruction: {}",
                transcript, _user_instruction
            )
        }
    }
}

// -----------------------------------------------------------------------------
// Registry / Helper
// -----------------------------------------------------------------------------

pub fn get_all_kits() -> Vec<Box<dyn PromptKit>> {
    vec![
        Box::new(DraftReplyKit),
        Box::new(SummarizeKit),
        Box::new(ActionItemsKit),
    ]
}

pub fn get_default_kit() -> Box<dyn PromptKit> {
    Box::new(DraftReplyKit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_kit_formatting() {
        let kit = DraftReplyKit;
        let p = kit.format_user_prompt("Hello", "Be nice");
        assert!(p.contains("Context:\nHello"));
        assert!(p.contains("Instruction: Be nice"));
    }

    #[test]
    fn summarize_kit_formatting() {
        let kit = SummarizeKit;
        let p = kit.format_user_prompt("Messages...", "");
        assert!(p.contains("Conversation:\nMessages..."));
    }
}
