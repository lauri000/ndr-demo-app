package social.innode.ndr.demo.ui.screens

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import social.innode.ndr.demo.core.AppManager

data class WelcomeUiState(
    val importValue: String = "",
    val isWorking: Boolean = false,
    val errorMessage: String? = null,
    val didLogin: Boolean = false,
)

class WelcomeViewModel(
    private val appManager: AppManager,
) : ViewModel() {
    private val mutableUiState = MutableStateFlow(WelcomeUiState())
    val uiState: StateFlow<WelcomeUiState> = mutableUiState.asStateFlow()

    init {
        viewModelScope.launch {
            appManager.state.collect { state ->
                mutableUiState.update { current ->
                    current.copy(
                        isWorking = state.busy.creatingAccount || state.busy.restoringSession,
                        errorMessage = state.toast,
                        didLogin = state.account != null,
                    )
                }
            }
        }
    }

    fun onImportValueChanged(value: String) {
        mutableUiState.value =
            mutableUiState.value.copy(
                importValue = value,
                errorMessage = null,
            )
    }

    fun generate() {
        mutableUiState.update { it.copy(errorMessage = null) }
        appManager.createAccount()
    }

    fun import() {
        mutableUiState.update { it.copy(errorMessage = null) }
        appManager.restoreSession(mutableUiState.value.importValue)
    }
}
