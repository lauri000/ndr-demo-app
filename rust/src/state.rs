#[derive(uniffi::Enum, Clone, Debug)]
pub enum Screen {
    Welcome,
    ChatList,
    NewChat,
    Chat { chat_id: String },
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
    pub creating_chat: bool,
    pub sending_message: bool,
    pub syncing_network: bool,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct AccountSnapshot {
    pub public_key_hex: String,
    pub npub: String,
    pub invite_url: String,
}

#[derive(uniffi::Enum, Clone, Debug)]
pub enum DeliveryState {
    Pending,
    Sent,
    Received,
    Failed,
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
    pub display_name: String,
    pub peer_npub: String,
    pub last_message_preview: Option<String>,
    pub unread_count: u64,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct CurrentChatSnapshot {
    pub chat_id: String,
    pub display_name: String,
    pub peer_npub: String,
    pub messages: Vec<ChatMessageSnapshot>,
}

#[derive(uniffi::Record, Clone, Debug)]
pub struct AppState {
    pub rev: u64,
    pub router: Router,
    pub account: Option<AccountSnapshot>,
    pub busy: BusyState,
    pub chat_list: Vec<ChatThreadSnapshot>,
    pub current_chat: Option<CurrentChatSnapshot>,
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
            busy: BusyState::default(),
            chat_list: Vec::new(),
            current_chat: None,
            toast: None,
        }
    }
}
