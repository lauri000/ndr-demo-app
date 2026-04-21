package social.innode.ndr.demo.ui.screens

import androidx.compose.runtime.Composable
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.AppState

@Composable
fun AwaitingDeviceApprovalScreen(
    appManager: AppManager,
    appState: AppState,
) {
    AddDeviceScreen(
        appManager = appManager,
        appState = appState,
        awaitingApproval = true,
    )
}
