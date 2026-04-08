package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.rust.isValidPeerInput
import social.innode.ndr.demo.rust.normalizePeerInput
import social.innode.ndr.demo.ui.components.IrisIcons
import social.innode.ndr.demo.ui.components.IrisPrimaryButton
import social.innode.ndr.demo.ui.components.IrisSectionCard
import social.innode.ndr.demo.ui.components.IrisSecondaryButton
import social.innode.ndr.demo.ui.theme.IrisTheme

@Composable
fun WelcomeScreen(
    uiState: WelcomeUiState,
    onImportValueChanged: (String) -> Unit,
    onLinkOwnerValueChanged: (String) -> Unit,
    onGenerateClick: () -> Unit,
    onImportClick: () -> Unit,
    onLinkExistingAccountClick: () -> Unit,
    onLoggedIn: () -> Unit,
) {
    var showScanner by remember { mutableStateOf(false) }
    val normalizedLinkValue = normalizePeerInput(uiState.linkOwnerValue)
    val isValidLinkValue =
        normalizedLinkValue.isNotBlank() && isValidPeerInput(normalizedLinkValue)

    LaunchedEffect(uiState.didLogin) {
        if (uiState.didLogin) {
            onLoggedIn()
        }
    }

    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        IrisSectionCard {
            Text(
                text = "Iris-style private chat",
                style = MaterialTheme.typography.headlineMedium,
            )
            Text(
                text = "This Android app keeps routing, protocol state, device authorization, and relay behavior in Rust. Android handles the visual shell, QR scanning, and secure account storage.",
                style = MaterialTheme.typography.bodyMedium,
                color = IrisTheme.palette.muted,
            )
            IrisPrimaryButton(
                text = if (uiState.isWorking) "Creating…" else "Generate new key",
                onClick = onGenerateClick,
                enabled = !uiState.isWorking,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .testTag("generateKeyButton"),
            )
        }

        IrisSectionCard {
            Text(
                text = "Import existing key",
                style = MaterialTheme.typography.titleMedium,
            )
            TextField(
                value = uiState.importValue,
                onValueChange = onImportValueChanged,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .testTag("importKeyField"),
                placeholder = {
                    Text(
                        text = "nsec or hex private key",
                        color = IrisTheme.palette.muted,
                    )
                },
                minLines = 3,
                enabled = !uiState.isWorking,
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
            IrisSecondaryButton(
                text = "Import existing key",
                onClick = onImportClick,
                enabled = !uiState.isWorking,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .testTag("importKeyButton"),
            )
        }

        IrisSectionCard {
            Text(
                text = "Link existing account",
                style = MaterialTheme.typography.titleMedium,
            )
            Text(
                text = "Scan the owner QR from your primary device. This device will publish its own invite, then wait for explicit approval in the owner-signed roster.",
                style = MaterialTheme.typography.bodyMedium,
                color = IrisTheme.palette.muted,
            )
            TextField(
                value = uiState.linkOwnerValue,
                onValueChange = onLinkOwnerValueChanged,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .testTag("linkOwnerInput"),
                placeholder = {
                    Text(
                        text = "Owner npub or hex",
                        color = IrisTheme.palette.muted,
                    )
                },
                isError = uiState.linkOwnerValue.isNotBlank() && !isValidLinkValue,
                enabled = !uiState.isWorking,
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

            if (uiState.linkOwnerValue.isNotBlank() && !isValidLinkValue) {
                Text(
                    text = "Scanned or pasted owner key is not valid.",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }

            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                IrisSecondaryButton(
                    text = "Scan owner QR",
                    onClick = { showScanner = true },
                    enabled = !uiState.isWorking,
                    modifier = Modifier.testTag("linkOwnerScanQrButton"),
                    icon = {
                        Icon(
                            imageVector = IrisIcons.ScanQr,
                            contentDescription = null,
                        )
                    },
                )
                IrisPrimaryButton(
                    text = "Link device",
                    onClick = onLinkExistingAccountClick,
                    enabled = isValidLinkValue && !uiState.isWorking,
                    modifier = Modifier.testTag("linkExistingAccountButton"),
                    icon = {
                        Icon(
                            imageVector = IrisIcons.Devices,
                            contentDescription = null,
                        )
                    },
                )
            }
        }

        uiState.errorMessage?.let { error ->
            IrisSectionCard {
                Text(
                    text = error,
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
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
                    "Scanned QR did not contain a valid owner public key."
                } else {
                    onLinkOwnerValueChanged(normalized)
                    showScanner = false
                    null
                }
            },
        )
    }
}
