package social.innode.ndr.demo.ui.navigation

import androidx.compose.runtime.Composable
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import social.innode.ndr.demo.core.AppContainer
import social.innode.ndr.demo.ui.screens.AccountScreen
import social.innode.ndr.demo.ui.screens.AccountViewModel
import social.innode.ndr.demo.ui.screens.DummyChatScreen
import social.innode.ndr.demo.ui.screens.DummyChatViewModel
import social.innode.ndr.demo.ui.screens.SplashScreen
import social.innode.ndr.demo.ui.screens.SplashViewModel
import social.innode.ndr.demo.ui.screens.WelcomeScreen
import social.innode.ndr.demo.ui.screens.WelcomeViewModel

private object Routes {
    const val Splash = "splash"
    const val Welcome = "welcome"
    const val Account = "account"
    const val DummyChat = "dummyChat"
}

@Composable
fun NdrApp(container: AppContainer) {
    val navController = rememberNavController()

    NavHost(
        navController = navController,
        startDestination = Routes.Splash,
    ) {
        composable(Routes.Splash) {
            val viewModel = remember { SplashViewModel(container.appManager) }
            val bootstrapState by viewModel.bootstrapState.collectAsStateWithLifecycle()
            SplashScreen(
                bootstrapState = bootstrapState,
                onNeedsLogin = {
                    navController.navigate(Routes.Welcome) {
                        popUpTo(Routes.Splash) { inclusive = true }
                    }
                },
                onLoggedIn = {
                    navController.navigate(Routes.Account) {
                        popUpTo(Routes.Splash) { inclusive = true }
                    }
                },
            )
        }

        composable(Routes.Welcome) {
            val viewModel = remember { WelcomeViewModel(container.appManager) }
            val uiState by viewModel.uiState.collectAsStateWithLifecycle()
            WelcomeScreen(
                uiState = uiState,
                onImportValueChanged = viewModel::onImportValueChanged,
                onGenerateClick = {
                    viewModel.generate()
                },
                onImportClick = {
                    viewModel.import()
                },
                onLoggedIn = {
                    navController.navigate(Routes.Account) {
                        popUpTo(Routes.Welcome) { inclusive = true }
                    }
                },
            )
        }

        composable(Routes.Account) {
            val viewModel = remember { AccountViewModel(container.appManager) }
            val uiState by viewModel.uiState.collectAsStateWithLifecycle()
            AccountScreen(
                uiState = uiState,
                onRevealClick = viewModel::revealNsec,
                onHideSecret = viewModel::hideNsec,
                onOpenChat = { navController.navigate(Routes.DummyChat) },
                onLogout = {
                    viewModel.logout()
                    navController.navigate(Routes.Welcome) {
                        popUpTo(Routes.Account) { inclusive = true }
                    }
                },
            )
        }

        composable(Routes.DummyChat) {
            val viewModel = remember { DummyChatViewModel(container.appManager) }
            val uiState by viewModel.uiState.collectAsStateWithLifecycle()
            DummyChatScreen(
                uiState = uiState,
                onPeerChanged = viewModel::updatePeer,
                onDraftChanged = viewModel::updateDraft,
                onSendClick = viewModel::send,
                onBack = { navController.popBackStack() },
            )
        }
    }
}
