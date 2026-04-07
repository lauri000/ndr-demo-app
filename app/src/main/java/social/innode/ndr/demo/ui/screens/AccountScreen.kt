package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp

@Composable
fun AccountScreen(
    uiState: AccountUiState,
    onRevealClick: () -> Unit,
    onHideSecret: () -> Unit,
    onOpenChat: () -> Unit,
    onLogout: () -> Unit,
) {
    val clipboard = LocalClipboardManager.current

    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(
            text = "Account ready",
            modifier = Modifier.testTag("accountReadyTitle"),
            style = MaterialTheme.typography.headlineMedium,
        )
        Text(
            text = "This device now runs a Rust-owned account and chat core. Open chat, enter the other device's npub on both phones, and the Rust app core will handle relay traffic, storage, and protocol state.",
            style = MaterialTheme.typography.bodyLarge,
        )

        Text("npub", style = MaterialTheme.typography.titleMedium)
        Text(uiState.npub, style = MaterialTheme.typography.bodyMedium)
        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(onClick = { clipboard.setText(AnnotatedString(uiState.npub)) }) {
                Text("Copy npub")
            }
            OutlinedButton(onClick = onRevealClick, modifier = Modifier.testTag("revealNsecButton")) {
                Text("Reveal nsec")
            }
        }

        Text("Public key hex", style = MaterialTheme.typography.titleMedium)
        Text(uiState.publicKeyHex, style = MaterialTheme.typography.bodyMedium)

        Button(
            onClick = onOpenChat,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("openDummyChatButton"),
        ) {
            Text("Open relay chat")
        }

        TextButton(onClick = onLogout) {
            Text("Log out")
        }
    }

    if (uiState.nsec != null) {
        AlertDialog(
            onDismissRequest = onHideSecret,
            title = { Text("Secret key") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text("Only reveal this when you explicitly want to export the account.")
                    Text(uiState.nsec, style = MaterialTheme.typography.bodyMedium)
                }
            },
            confirmButton = {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(onClick = { clipboard.setText(AnnotatedString(uiState.nsec)) }) {
                        Text("Copy")
                    }
                    TextButton(onClick = onHideSecret) {
                        Text("Done")
                    }
                }
            },
        )
    }
}
