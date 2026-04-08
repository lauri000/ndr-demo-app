package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.AppAction
import social.innode.ndr.demo.rust.AppState
import social.innode.ndr.demo.rust.isValidPeerInput
import social.innode.ndr.demo.rust.normalizePeerInput
import social.innode.ndr.demo.ui.components.IrisIcons
import social.innode.ndr.demo.ui.components.IrisPrimaryButton
import social.innode.ndr.demo.ui.components.IrisSectionCard
import social.innode.ndr.demo.ui.components.IrisSecondaryButton
import social.innode.ndr.demo.ui.components.IrisTopBar
import social.innode.ndr.demo.ui.theme.IrisTheme

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
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            IrisTopBar(
                title = "New chat",
                onBack = {
                    appManager.dispatch(
                        AppAction.UpdateScreenStack(appState.router.screenStack.dropLast(1)),
                    )
                },
            )
        },
    ) { padding ->
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            IrisSectionCard {
                Text(
                    text = "Start a direct conversation",
                    style = MaterialTheme.typography.titleLarge,
                )
                Text(
                    text = "Paste an npub, paste a 64-character hex key, or scan a QR from another device. This mirrors the lightweight start-chat flow in Iris web.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = IrisTheme.palette.muted,
                )
            }

            IrisSectionCard {
                Text(
                    text = "Peer key",
                    style = MaterialTheme.typography.titleMedium,
                )
                TextField(
                    value = peerInput,
                    onValueChange = { peerInput = it },
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .testTag("newChatPeerInput"),
                    placeholder = {
                        Text(
                            text = "npub, hex, or nostr:...",
                            color = IrisTheme.palette.muted,
                        )
                    },
                    singleLine = false,
                    minLines = 2,
                    colors =
                        TextFieldDefaults.colors(
                            focusedContainerColor = IrisTheme.palette.panelAlt,
                            unfocusedContainerColor = IrisTheme.palette.panelAlt,
                            disabledContainerColor = IrisTheme.palette.panelAlt,
                            focusedIndicatorColor = Color.Transparent,
                            unfocusedIndicatorColor = Color.Transparent,
                            disabledIndicatorColor = Color.Transparent,
                        ),
                )

                if (peerInput.isNotBlank() && !isValidPeer) {
                    Text(
                        text = "Not a valid nostr public key.",
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }

                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    IrisSecondaryButton(
                        text = "Paste",
                        onClick = {
                            peerInput = normalizePeerInput(clipboard.getText()?.text.orEmpty())
                        },
                        modifier = Modifier.testTag("newChatPasteButton"),
                        icon = {
                            Icon(
                                imageVector = IrisIcons.Copy,
                                contentDescription = null,
                            )
                        },
                    )
                    IrisSecondaryButton(
                        text = "Scan QR",
                        onClick = { showScanner = true },
                        modifier = Modifier.testTag("newChatScanQrButton"),
                        icon = {
                            Icon(
                                imageVector = IrisIcons.ScanQr,
                                contentDescription = null,
                            )
                        },
                    )
                }

                IrisPrimaryButton(
                    text = if (appState.busy.creatingChat) "Creating…" else "Open chat",
                    onClick = { appManager.createChat(normalizedInput) },
                    enabled = isValidPeer && !appState.busy.creatingChat,
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .testTag("newChatStartButton"),
                    icon = {
                        Icon(
                            imageVector = IrisIcons.NewChat,
                            contentDescription = null,
                        )
                    },
                )
            }
        }
    }

    if (showScanner) {
        QrScannerDialog(
            onDismiss = { showScanner = false },
            onScanned = { scanned ->
                val normalized = normalizePeerInput(scanned)
                if (!isValidPeerInput(normalized)) {
                    "Scanned QR did not contain a valid public key."
                } else {
                    peerInput = normalized
                    showScanner = false
                    null
                }
            },
        )
    }
}
