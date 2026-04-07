#[derive(uniffi::Enum, Clone, Debug)]
pub enum AppAction {
    CreateAccount,
    RestoreSession { nsec: String },
    Logout,
    OpenChat { peer_input: String },
    CloseChat,
    SendMessage { peer_input: String, text: String },
}
