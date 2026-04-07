package social.innode.ndr.demo.ui.screens

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import social.innode.ndr.demo.core.AppManager

data class AccountUiState(
    val npub: String = "",
    val publicKeyHex: String = "",
    val nsec: String? = null,
)

class AccountViewModel(
    private val appManager: AppManager,
) : ViewModel() {
    private val mutableUiState = MutableStateFlow(AccountUiState())
    val uiState: StateFlow<AccountUiState> = mutableUiState.asStateFlow()

    init {
        viewModelScope.launch {
            appManager.state.collect { state ->
                val account = state.account
                mutableUiState.update { current ->
                    current.copy(
                        npub = account?.npub.orEmpty(),
                        publicKeyHex = account?.publicKeyHex.orEmpty(),
                    )
                }
            }
        }
    }

    fun revealNsec() {
        viewModelScope.launch {
            mutableUiState.update { it.copy(nsec = appManager.exportNsec()) }
        }
    }

    fun hideNsec() {
        mutableUiState.update { it.copy(nsec = null) }
    }

    fun logout() {
        viewModelScope.launch {
            appManager.logout()
        }
    }
}
