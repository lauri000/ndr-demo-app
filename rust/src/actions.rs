use crate::state::Screen;

#[derive(uniffi::Enum, Clone, Debug)]
pub enum AppAction {
    CreateAccount,
    RestoreSession { owner_nsec: String },
    RestoreAccountBundle {
        owner_nsec: Option<String>,
        owner_pubkey_hex: String,
        device_nsec: String,
    },
    StartLinkedDevice { owner_input: String },
    Logout,
    CreateChat { peer_input: String },
    CreateGroup { name: String, member_inputs: Vec<String> },
    OpenChat { chat_id: String },
    SendMessage { chat_id: String, text: String },
    UpdateGroupName { group_id: String, name: String },
    AddGroupMembers { group_id: String, member_inputs: Vec<String> },
    RemoveGroupMember { group_id: String, owner_pubkey_hex: String },
    AddAuthorizedDevice { device_input: String },
    RemoveAuthorizedDevice { device_pubkey_hex: String },
    AcknowledgeRevokedDevice,
    PushScreen { screen: Screen },
    UpdateScreenStack { stack: Vec<Screen> },
}
