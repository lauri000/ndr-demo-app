package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.qr.DeviceApprovalQr
import social.innode.ndr.demo.rust.AppState

@Composable
fun AwaitingDeviceApprovalScreen(
    appManager: AppManager,
    appState: AppState,
) {
    val account = appState.account ?: return
    val clipboard = LocalClipboardManager.current
    val approvalQrValue =
        remember(account.npub, account.deviceNpub) {
            DeviceApprovalQr.encode(
                ownerInput = account.npub,
                deviceInput = account.deviceNpub,
            )
        }
    val qrBitmap =
        remember(approvalQrValue) {
            createQrBitmap(approvalQrValue, size = 768)
        }

    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .padding(24.dp)
                .testTag("awaitingApprovalScreen"),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(
            text = "Waiting for device approval",
            style = MaterialTheme.typography.headlineMedium,
        )
        Text(
            text = "This device has already published its own invite. From the primary device, open Manage devices and scan the QR below to authorize it immediately.",
            style = MaterialTheme.typography.bodyLarge,
        )

        Text("Owner npub", style = MaterialTheme.typography.titleMedium)
        Text(
            text = account.npub,
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag("awaitingApprovalOwnerNpub"),
        )

        Text("This device npub", style = MaterialTheme.typography.titleMedium)
        Text(
            text = account.deviceNpub,
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.testTag("awaitingApprovalDeviceNpub"),
        )

        Box(
            modifier = Modifier.fillMaxWidth(),
            contentAlignment = Alignment.Center,
        ) {
            if (qrBitmap != null) {
                Image(
                    bitmap = qrBitmap.asImageBitmap(),
                    contentDescription = "Device npub QR code",
                    modifier =
                        Modifier
                            .size(260.dp)
                            .testTag("awaitingApprovalDeviceQrCode"),
                )
            }
        }

        TextButton(
            onClick = { clipboard.setText(AnnotatedString(account.deviceNpub)) },
            modifier = Modifier.testTag("awaitingApprovalCopyDeviceButton"),
        ) {
            Text("Copy device npub")
        }

        TextButton(onClick = appManager::logout) {
            Text("Logout")
        }
    }
}
