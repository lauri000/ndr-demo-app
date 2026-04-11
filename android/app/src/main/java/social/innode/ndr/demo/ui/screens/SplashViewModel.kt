package social.innode.ndr.demo.ui.screens

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.StateFlow
import social.innode.ndr.demo.account.AccountBootstrapState
import social.innode.ndr.demo.core.AppManager

class SplashViewModel(
    appManager: AppManager,
) : ViewModel() {
    val bootstrapState: StateFlow<AccountBootstrapState> = appManager.bootstrapState
}
