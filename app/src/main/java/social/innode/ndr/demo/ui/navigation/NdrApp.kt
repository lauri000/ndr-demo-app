package social.innode.ndr.demo.ui.navigation

import android.widget.Toast
import androidx.activity.compose.BackHandler
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import social.innode.ndr.demo.account.AccountBootstrapState
import social.innode.ndr.demo.core.AppContainer
import social.innode.ndr.demo.rust.AppAction
import social.innode.ndr.demo.rust.Screen
import social.innode.ndr.demo.ui.screens.ChatListScreen
import social.innode.ndr.demo.ui.screens.ChatScreen
import social.innode.ndr.demo.ui.screens.DeviceRevokedScreen
import social.innode.ndr.demo.ui.screens.DeviceRosterScreen
import social.innode.ndr.demo.ui.screens.NewChatScreen
import social.innode.ndr.demo.ui.screens.SplashScreen
import social.innode.ndr.demo.ui.screens.SplashViewModel
import social.innode.ndr.demo.ui.screens.AwaitingDeviceApprovalScreen
import social.innode.ndr.demo.ui.screens.WelcomeScreen
import social.innode.ndr.demo.ui.screens.WelcomeViewModel

@Composable
fun NdrApp(container: AppContainer) {
    val appManager = container.appManager
    val splashViewModel = remember { SplashViewModel(appManager) }
    val welcomeViewModel = remember { WelcomeViewModel(appManager) }
    val bootstrapState by splashViewModel.bootstrapState.collectAsStateWithLifecycle()
    val appState by appManager.state.collectAsStateWithLifecycle()
    val welcomeUiState by welcomeViewModel.uiState.collectAsStateWithLifecycle()
    val context = LocalContext.current

    LaunchedEffect(appState.toast) {
        val message = appState.toast ?: return@LaunchedEffect
        Toast.makeText(context, message, Toast.LENGTH_LONG).show()
    }

    when (bootstrapState) {
        AccountBootstrapState.Loading -> {
            SplashScreen(
                bootstrapState = bootstrapState,
                onNeedsLogin = {},
                onLoggedIn = {},
            )
        }

        AccountBootstrapState.NeedsLogin -> {
            WelcomeScreen(
                uiState = welcomeUiState,
                onImportValueChanged = welcomeViewModel::onImportValueChanged,
                onLinkOwnerValueChanged = welcomeViewModel::onLinkOwnerValueChanged,
                onGenerateClick = welcomeViewModel::generate,
                onImportClick = welcomeViewModel::import,
                onLinkExistingAccountClick = welcomeViewModel::linkExistingAccount,
                onLoggedIn = {},
            )
        }

        is AccountBootstrapState.LoggedIn -> {
            val router = appState.router
            BackHandler(enabled = router.screenStack.isNotEmpty()) {
                appManager.dispatch(AppAction.UpdateScreenStack(router.screenStack.dropLast(1)))
            }

            when (val screen = router.screenStack.lastOrNull() ?: router.defaultScreen) {
                Screen.Welcome -> {
                    WelcomeScreen(
                        uiState = welcomeUiState,
                        onImportValueChanged = welcomeViewModel::onImportValueChanged,
                        onLinkOwnerValueChanged = welcomeViewModel::onLinkOwnerValueChanged,
                        onGenerateClick = welcomeViewModel::generate,
                        onImportClick = welcomeViewModel::import,
                        onLinkExistingAccountClick = welcomeViewModel::linkExistingAccount,
                        onLoggedIn = {},
                    )
                }

                Screen.ChatList -> {
                    ChatListScreen(appManager = appManager, appState = appState)
                }

                Screen.NewChat -> {
                    NewChatScreen(appManager = appManager, appState = appState)
                }

                Screen.DeviceRoster -> {
                    DeviceRosterScreen(appManager = appManager, appState = appState)
                }

                Screen.AwaitingDeviceApproval -> {
                    AwaitingDeviceApprovalScreen(appManager = appManager, appState = appState)
                }

                Screen.DeviceRevoked -> {
                    DeviceRevokedScreen(appManager = appManager, appState = appState)
                }

                is Screen.Chat -> {
                    ChatScreen(
                        appManager = appManager,
                        appState = appState,
                        chatId = screen.chatId,
                    )
                }
            }
        }
    }
}
