use crate::state::Screen;

#[derive(uniffi::Enum, Clone, Debug)]
pub enum AppAction {
    CreateAccount,
    RestoreSession { nsec: String },
    Logout,
    CreateChat { peer_input: String },
    OpenChat { chat_id: String },
    SendMessage { chat_id: String, text: String },
    PushScreen { screen: Screen },
    UpdateScreenStack { stack: Vec<Screen> },
}
