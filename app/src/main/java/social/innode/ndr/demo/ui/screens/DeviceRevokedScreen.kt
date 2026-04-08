package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.AppState

@Composable
fun DeviceRevokedScreen(
    appManager: AppManager,
    appState: AppState,
) {
    val account = appState.account

    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .padding(24.dp)
                .testTag("deviceRevokedScreen"),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(
            text = "Device removed",
            style = MaterialTheme.typography.headlineMedium,
        )
        Text(
            text =
                "This device is no longer in the owner-signed roster. Messaging is blocked until you log out or re-link the device from a primary device.",
            style = MaterialTheme.typography.bodyLarge,
        )
        account?.let {
            Text(
                text = "Owner: ${it.npub}",
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                text = "Device: ${it.deviceNpub}",
                style = MaterialTheme.typography.bodyMedium,
            )
        }
        Button(
            onClick = appManager::logout,
            modifier = Modifier.testTag("deviceRevokedLogoutButton"),
        ) {
            Text("Logout")
        }
    }
}
