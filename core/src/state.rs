#[derive(uniffi::Enum, Clone, Debug)]
pub enum Screen {
    Welcome,
    CreateAccount,
    RestoreAccount,
    AddDevice,
    ChatList,
    NewChat,
    NewGroup,
    Chat { chat_id: String },
    GroupDetails { group_id: String },
    DeviceRoster,
    AwaitingDeviceApproval,
    DeviceRevoked,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct Router {
    pub default_screen: Screen,
    pub screen_stack: Vec<Screen>,
}

#[derive(uniffi::Record, Clone, Debug, Default)]
pub struct BusyState {
    pub creating_account: bool,
    pub restoring_session: bool,
    pub linking_device: bool,
    pub creating_chat: bool,
    pub creating_group: bool,
    pub sending_message: bool,
    pub updating_roster: bool,
    pub updating_group: bool,
    pub syncing_network: bool,
}

#[derive(uniffi::Enum, Clone, Debug)]
pub enum DeviceAuthorizationState {
    Authorized,
    AwaitingApproval,
    Revoked,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct AccountSnapshot {
    pub public_key_hex: String,
    pub npub: String,
    pub display_name: String,
    pub device_public_key_hex: String,
    pub device_npub: String,
    pub has_owner_signing_authority: bool,
    pub authorization_state: DeviceAuthorizationState,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct DeviceEntrySnapshot {
    pub device_pubkey_hex: String,
    pub device_npub: String,
    pub is_current_device: bool,
    pub is_authorized: bool,
    pub is_stale: bool,
    pub last_activity_secs: Option<u64>,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct DeviceRosterSnapshot {
    pub owner_public_key_hex: String,
    pub owner_npub: String,
    pub current_device_public_key_hex: String,
    pub current_device_npub: String,
    pub can_manage_devices: bool,
    pub authorization_state: DeviceAuthorizationState,
    pub devices: Vec<DeviceEntrySnapshot>,
}

#[derive(uniffi::Enum, Clone, Debug)]
pub enum DeliveryState {
    Pending,
    Sent,
    Received,
    Failed,
}

#[derive(uniffi::Enum, Clone, Debug)]
pub enum ChatKind {
    Direct,
    Group,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct ChatMessageSnapshot {
    pub id: String,
    pub chat_id: String,
    pub author: String,
    pub body: String,
    pub is_outgoing: bool,
    pub created_at_secs: u64,
    pub delivery: DeliveryState,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct ChatThreadSnapshot {
    pub chat_id: String,
    pub kind: ChatKind,
    pub display_name: String,
    pub subtitle: Option<String>,
    pub member_count: u64,
    pub last_message_preview: Option<String>,
    pub last_message_at_secs: Option<u64>,
    pub last_message_is_outgoing: Option<bool>,
    pub last_message_delivery: Option<DeliveryState>,
    pub unread_count: u64,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct CurrentChatSnapshot {
    pub chat_id: String,
    pub kind: ChatKind,
    pub display_name: String,
    pub subtitle: Option<String>,
    pub group_id: Option<String>,
    pub member_count: u64,
    pub messages: Vec<ChatMessageSnapshot>,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct GroupMemberSnapshot {
    pub owner_pubkey_hex: String,
    pub display_name: String,
    pub npub: String,
    pub is_admin: bool,
    pub is_creator: bool,
    pub is_local_owner: bool,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct GroupDetailsSnapshot {
    pub group_id: String,
    pub name: String,
    pub created_by_display_name: String,
    pub created_by_npub: String,
    pub can_manage: bool,
    pub revision: u64,
    pub members: Vec<GroupMemberSnapshot>,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct AppState {
    pub rev: u64,
    pub router: Router,
    pub account: Option<AccountSnapshot>,
    pub device_roster: Option<DeviceRosterSnapshot>,
    pub busy: BusyState,
    pub chat_list: Vec<ChatThreadSnapshot>,
    pub current_chat: Option<CurrentChatSnapshot>,
    pub group_details: Option<GroupDetailsSnapshot>,
    pub toast: Option<String>,
}

impl AppState {
    pub fn empty() -> Self {
        Self {
            rev: 0,
            router: Router {
                default_screen: Screen::Welcome,
                screen_stack: Vec::new(),
            },
            account: None,
            device_roster: None,
            busy: BusyState::default(),
            chat_list: Vec::new(),
            current_chat: None,
            group_details: None,
            toast: None,
        }
    }
}
