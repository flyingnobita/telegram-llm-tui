#[derive(Debug)]
pub enum UiCommand {
    UpdateComposer(String),
    ShowNotification(String),
    LlmResponse(String),
}
