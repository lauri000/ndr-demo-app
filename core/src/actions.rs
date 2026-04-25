use crate::state::{OutgoingAttachment, Screen};

#[derive(uniffi::Enum, Clone, Debug)]
pub enum AppAction {
    CreateAccount {
        name: String,
    },
    UpdateProfileMetadata {
        name: String,
        picture_url: Option<String>,
    },
    RestoreSession {
        owner_nsec: String,
    },
    RestoreAccountBundle {
        owner_nsec: Option<String>,
        owner_pubkey_hex: String,
        device_nsec: String,
    },
    StartLinkedDevice {
        owner_input: String,
    },
    AppForegrounded,
    Logout,
    CreateChat {
        peer_input: String,
    },
    CreateGroup {
        name: String,
        member_inputs: Vec<String>,
    },
    OpenChat {
        chat_id: String,
    },
    SendMessage {
        chat_id: String,
        text: String,
    },
    SendAttachment {
        chat_id: String,
        file_path: String,
        filename: String,
        caption: String,
    },
    SendAttachments {
        chat_id: String,
        attachments: Vec<OutgoingAttachment>,
        caption: String,
    },
    ToggleReaction {
        chat_id: String,
        message_id: String,
        emoji: String,
    },
    SendTyping {
        chat_id: String,
    },
    SetTypingIndicatorsEnabled {
        enabled: bool,
    },
    SetDesktopNotificationsEnabled {
        enabled: bool,
    },
    SetStartupAtLoginEnabled {
        enabled: bool,
    },
    MarkMessagesSeen {
        chat_id: String,
        message_ids: Vec<String>,
    },
    DeleteLocalMessage {
        chat_id: String,
        message_id: String,
    },
    UpdateGroupName {
        group_id: String,
        name: String,
    },
    AddGroupMembers {
        group_id: String,
        member_inputs: Vec<String>,
    },
    RemoveGroupMember {
        group_id: String,
        owner_pubkey_hex: String,
    },
    AddAuthorizedDevice {
        device_input: String,
    },
    RemoveAuthorizedDevice {
        device_pubkey_hex: String,
    },
    AcknowledgeRevokedDevice,
    PushScreen {
        screen: Screen,
    },
    UpdateScreenStack {
        stack: Vec<Screen>,
    },
}
