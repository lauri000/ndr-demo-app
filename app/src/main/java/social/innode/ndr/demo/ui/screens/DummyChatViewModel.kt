package social.innode.ndr.demo.ui.screens

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.AppState

data class DummyChatUiState(
    val peerNpub: String = "",
    val draft: String = "",
    val messages: List<ChatBubbleUi> = emptyList(),
    val errorMessage: String? = null,
)

data class ChatBubbleUi(
    val id: Long,
    val text: String,
    val isOutgoing: Boolean,
)

class DummyChatViewModel(
    private val appManager: AppManager,
) : ViewModel() {
    private val mutableUiState = MutableStateFlow(DummyChatUiState())
    val uiState: StateFlow<DummyChatUiState> = mutableUiState.asStateFlow()

    init {
        viewModelScope.launch {
            appManager.state.collect { state ->
                mutableUiState.update { current ->
                    current.copy(
                        peerNpub = current.peerNpub.ifBlank { state.currentChat?.peerInput.orEmpty() },
                        messages = toChatMessages(state),
                        errorMessage = state.toast,
                    )
                }
            }
        }
    }

    fun updatePeer(value: String) {
        val peer = value.trim()
        mutableUiState.update { current ->
            current.copy(
                peerNpub = peer,
                messages = toChatMessages(appManager.state.value),
                errorMessage = null,
            )
        }
        when {
            peer.isBlank() -> appManager.closeChat()
            looksLikePeerInput(peer) -> appManager.openChat(peer)
        }
    }

    fun updateDraft(value: String) {
        mutableUiState.update { it.copy(draft = value) }
    }

    fun send() {
        val draft = mutableUiState.value.draft
        if (draft.isBlank()) {
            return
        }
        if (mutableUiState.value.peerNpub.isBlank()) {
            mutableUiState.update { state ->
                state.copy(errorMessage = "Enter the peer npub first.")
            }
            return
        }
        appManager.sendText(mutableUiState.value.peerNpub, draft)
        mutableUiState.update { state ->
            state.copy(draft = "", errorMessage = null)
        }
    }

    private fun toChatMessages(state: AppState): List<ChatBubbleUi> {
        val activeChat = state.currentChat
        if (activeChat == null) {
            return emptyList()
        }
        return activeChat.messages.map { message ->
                ChatBubbleUi(
                    id = message.id.toLongOrNull() ?: 0L,
                    text = message.body,
                    isOutgoing = message.isOutgoing,
                )
            }
    }

    private fun looksLikePeerInput(value: String): Boolean {
        val trimmed = value.trim()
        return trimmed.startsWith("npub1") || trimmed.matches(Regex("[0-9a-fA-F]{64}"))
    }
}
