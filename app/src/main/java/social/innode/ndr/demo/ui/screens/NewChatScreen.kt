package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.AppAction
import social.innode.ndr.demo.rust.AppState
import social.innode.ndr.demo.rust.isValidPeerInput
import social.innode.ndr.demo.rust.normalizePeerInput

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun NewChatScreen(
    appManager: AppManager,
    appState: AppState,
) {
    val clipboard = LocalClipboardManager.current
    var peerInput by remember { mutableStateOf("") }
    var showScanner by remember { mutableStateOf(false) }
    val normalizedInput = normalizePeerInput(peerInput)
    val isValidPeer = normalizedInput.isNotBlank() && isValidPeerInput(normalizedInput)

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("New chat") },
                navigationIcon = {
                    TextButton(
                        onClick = {
                            appManager.dispatch(
                                AppAction.UpdateScreenStack(appState.router.screenStack.dropLast(1)),
                            )
                        },
                    ) {
                        Text("Back")
                    }
                },
            )
        },
    ) { padding ->
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Text(
                text = "Enter an `npub`, 64-character hex pubkey, or scan a QR code.",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )

            OutlinedTextField(
                value = peerInput,
                onValueChange = { peerInput = it },
                label = { Text("Peer key") },
                singleLine = true,
                isError = peerInput.isNotBlank() && !isValidPeer,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .testTag("newChatPeerInput"),
            )

            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                TextButton(
                    onClick = {
                        peerInput = normalizePeerInput(clipboard.getText()?.text.orEmpty())
                    },
                    modifier = Modifier.testTag("newChatPasteButton"),
                ) {
                    Text("Paste")
                }
                TextButton(
                    onClick = { showScanner = true },
                    modifier = Modifier.testTag("newChatScanQrButton"),
                ) {
                    Text("Scan QR")
                }
            }

            if (peerInput.isNotBlank() && !isValidPeer) {
                Text(
                    text = "Not a valid nostr public key.",
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }

            Button(
                onClick = { appManager.createChat(normalizedInput) },
                enabled = isValidPeer && !appState.busy.creatingChat,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .testTag("newChatStartButton"),
            ) {
                if (appState.busy.creatingChat) {
                    CircularProgressIndicator(strokeWidth = 2.dp)
                } else {
                    Text("Start chat")
                }
            }
        }
    }

    if (showScanner) {
        QrScannerDialog(
            onDismiss = { showScanner = false },
            onScanned = { scanned ->
                peerInput = normalizePeerInput(scanned)
                showScanner = false
            },
        )
    }
}
