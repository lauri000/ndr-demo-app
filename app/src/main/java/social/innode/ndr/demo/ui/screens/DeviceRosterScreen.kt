package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.qr.DeviceApprovalQr
import social.innode.ndr.demo.rust.AppAction
import social.innode.ndr.demo.rust.AppState
import social.innode.ndr.demo.rust.isValidPeerInput
import social.innode.ndr.demo.rust.normalizePeerInput

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun DeviceRosterScreen(
    appManager: AppManager,
    appState: AppState,
) {
    val roster = appState.deviceRoster
    var deviceInput by remember { mutableStateOf("") }
    var showScanner by remember { mutableStateOf(false) }
    val resolvedInput = roster?.let { resolveDeviceAuthorizationInput(deviceInput, it.ownerNpub, it.ownerPublicKeyHex) }
    val normalizedInput = resolvedInput?.deviceInput.orEmpty()
    val canAddDevice =
        roster?.canManageDevices == true &&
            normalizedInput.isNotBlank() &&
            !appState.busy.updatingRoster

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Manage devices") },
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
        if (roster == null) {
            Column(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .padding(padding)
                        .padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Text("Loading roster…")
            }
            return@Scaffold
        }

        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                text = "Owner",
                style = MaterialTheme.typography.titleMedium,
            )
            Text(
                text = roster.ownerNpub,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.testTag("deviceRosterOwnerNpub"),
            )

            Text(
                text = "Current device",
                style = MaterialTheme.typography.titleMedium,
            )
            Text(
                text = roster.currentDeviceNpub,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.testTag("deviceRosterCurrentDeviceNpub"),
            )

            if (roster.canManageDevices) {
                Text(
                    text = "Scan the QR from the device waiting for approval, or paste its device key or approval code.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )

                OutlinedTextField(
                    value = deviceInput,
                    onValueChange = { deviceInput = it },
                    label = { Text("Device npub, hex, or approval code") },
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .testTag("deviceRosterAddInput"),
                    isError = deviceInput.isNotBlank() && resolvedInput?.errorMessage != null,
                )

                resolvedInput?.errorMessage?.let { error ->
                    Text(
                        text = error,
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                }

                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    TextButton(
                        onClick = { showScanner = true },
                        modifier = Modifier.testTag("deviceRosterScanButton"),
                    ) {
                        Text("Scan QR")
                    }

                    Button(
                        onClick = {
                            appManager.addAuthorizedDevice(normalizedInput)
                            deviceInput = ""
                        },
                        enabled = canAddDevice,
                        modifier = Modifier.testTag("deviceRosterAddButton"),
                    ) {
                        if (appState.busy.updatingRoster) {
                            CircularProgressIndicator(strokeWidth = 2.dp)
                        } else {
                            Text("Authorize device")
                        }
                    }
                }
            } else {
                Text(
                    text = "This device can view the roster but cannot publish roster changes.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }

            Text(
                text = "Authorized devices",
                style = MaterialTheme.typography.titleMedium,
            )

            LazyColumn(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .weight(1f),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                items(roster.devices, key = { it.devicePubkeyHex }) { device ->
                    Column(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .testTag("deviceRosterRow-${device.devicePubkeyHex.take(12)}"),
                        verticalArrangement = Arrangement.spacedBy(6.dp),
                    ) {
                        Text(
                            text =
                                when {
                                    device.isCurrentDevice -> "${device.deviceNpub} (this device)"
                                    else -> device.deviceNpub
                                },
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        Text(
                            text =
                                buildString {
                                    append(if (device.isAuthorized) "Authorized" else "Pending")
                                    if (device.isStale) append(" • stale")
                                },
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        if (roster.canManageDevices && !device.isCurrentDevice) {
                            TextButton(
                                onClick = { appManager.removeAuthorizedDevice(device.devicePubkeyHex) },
                                modifier =
                                    Modifier.testTag(
                                        "deviceRosterRemove-${device.devicePubkeyHex.take(12)}",
                                    ),
                            ) {
                                Text("Remove")
                            }
                        }
                    }
                }
            }
        }
    }

    if (showScanner && roster != null) {
        QrScannerDialog(
            onDismiss = { showScanner = false },
            onScanned = { scanned ->
                val resolved = resolveDeviceAuthorizationInput(scanned, roster.ownerNpub, roster.ownerPublicKeyHex)
                if (resolved.errorMessage != null) {
                    resolved.errorMessage
                } else {
                    appManager.addAuthorizedDevice(resolved.deviceInput)
                    deviceInput = ""
                    showScanner = false
                    null
                }
            },
        )
    }
}

private data class ResolvedDeviceAuthorizationInput(
    val deviceInput: String,
    val errorMessage: String?,
)

private fun resolveDeviceAuthorizationInput(
    rawInput: String,
    ownerNpub: String,
    ownerPublicKeyHex: String,
): ResolvedDeviceAuthorizationInput {
    val trimmed = rawInput.trim()
    if (trimmed.isEmpty()) {
        return ResolvedDeviceAuthorizationInput(deviceInput = "", errorMessage = null)
    }

    val approvalPayload = DeviceApprovalQr.decode(trimmed)
    if (approvalPayload != null) {
        val normalizedOwner = normalizePeerInput(approvalPayload.ownerInput)
        val acceptedOwnerInputs =
            setOf(
                normalizePeerInput(ownerNpub),
                normalizePeerInput(ownerPublicKeyHex),
            )
        if (!isValidPeerInput(normalizedOwner) || normalizedOwner !in acceptedOwnerInputs) {
            return ResolvedDeviceAuthorizationInput(
                deviceInput = "",
                errorMessage = "This approval QR belongs to a different owner.",
            )
        }

        val normalizedDeviceInput = normalizePeerInput(approvalPayload.deviceInput)
        return if (isValidPeerInput(normalizedDeviceInput)) {
            ResolvedDeviceAuthorizationInput(deviceInput = normalizedDeviceInput, errorMessage = null)
        } else {
            ResolvedDeviceAuthorizationInput(
                deviceInput = "",
                errorMessage = "Scanned approval QR did not contain a valid device public key.",
            )
        }
    }

    val normalizedInput = normalizePeerInput(trimmed)
    return if (isValidPeerInput(normalizedInput)) {
        ResolvedDeviceAuthorizationInput(deviceInput = normalizedInput, errorMessage = null)
    } else {
        ResolvedDeviceAuthorizationInput(
            deviceInput = "",
            errorMessage = "Not a valid device public key.",
        )
    }
}
