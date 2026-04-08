package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.rust.isValidPeerInput
import social.innode.ndr.demo.rust.normalizePeerInput

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
                .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(
            text = "Device-to-device bootstrap",
            style = MaterialTheme.typography.headlineMedium,
        )
        Text(
            text = "Generate a fresh primary account, import an existing owner key, or link a new device to an existing owner npub. The Rust app core owns relay connections, persistence, routing, and protocol state. Android renders UI, scans QR codes, and stores the encrypted account bundle.",
            style = MaterialTheme.typography.bodyLarge,
        )

        Button(
            onClick = onGenerateClick,
            enabled = !uiState.isWorking,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("generateKeyButton"),
            contentPadding = PaddingValues(vertical = 16.dp),
        ) {
            if (uiState.isWorking) {
                CircularProgressIndicator(strokeWidth = 2.dp)
            } else {
                Text("Generate new key")
            }
        }

        OutlinedTextField(
            value = uiState.importValue,
            onValueChange = onImportValueChanged,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("importKeyField"),
            label = { Text("nsec or hex private key") },
            minLines = 3,
            enabled = !uiState.isWorking,
        )

        OutlinedButton(
            onClick = onImportClick,
            enabled = !uiState.isWorking,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("importKeyButton"),
            contentPadding = PaddingValues(vertical = 16.dp),
        ) {
            Text("Import existing key")
        }

        Text(
            text = "Link existing account",
            style = MaterialTheme.typography.titleMedium,
        )
        Text(
            text = "Scan or paste the owner npub from your primary device. This new device will publish its own invite and wait for approval in the device roster.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        OutlinedTextField(
            value = uiState.linkOwnerValue,
            onValueChange = onLinkOwnerValueChanged,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("linkOwnerInput"),
            label = { Text("Owner npub or hex") },
            singleLine = true,
            isError = uiState.linkOwnerValue.isNotBlank() && !isValidLinkValue,
            enabled = !uiState.isWorking,
        )

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(
                onClick = { showScanner = true },
                enabled = !uiState.isWorking,
                modifier = Modifier.testTag("linkOwnerScanQrButton"),
            ) {
                Text("Scan owner QR")
            }

            Button(
                onClick = onLinkExistingAccountClick,
                enabled = isValidLinkValue && !uiState.isWorking,
                modifier = Modifier.testTag("linkExistingAccountButton"),
            ) {
                Text("Link existing account")
            }
        }

        uiState.errorMessage?.let { error ->
            Text(
                text = error,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyMedium,
            )
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
