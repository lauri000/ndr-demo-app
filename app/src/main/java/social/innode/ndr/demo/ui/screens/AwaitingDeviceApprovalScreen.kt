package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
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
import social.innode.ndr.demo.ui.components.IrisIcons
import social.innode.ndr.demo.ui.components.IrisPrimaryButton
import social.innode.ndr.demo.ui.components.IrisSectionCard
import social.innode.ndr.demo.ui.components.IrisSecondaryButton
import social.innode.ndr.demo.ui.theme.IrisTheme

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
                .padding(16.dp)
                .testTag("awaitingApprovalScreen"),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        IrisSectionCard {
            Text(
                text = "Waiting for approval",
                style = MaterialTheme.typography.headlineSmall,
            )
            Text(
                text = "This device already published its own invite. On the primary device, open Manage devices and approve it there. If it does not appear, scan the QR below as fallback.",
                style = MaterialTheme.typography.bodyMedium,
                color = IrisTheme.palette.muted,
            )
            Box(
                modifier = Modifier.fillMaxWidth(),
                contentAlignment = Alignment.Center,
            ) {
                if (qrBitmap != null) {
                    Image(
                        bitmap = qrBitmap.asImageBitmap(),
                        contentDescription = "Device approval QR code",
                        modifier =
                            Modifier
                                .size(260.dp)
                                .testTag("awaitingApprovalDeviceQrCode"),
                    )
                }
            }
        }

        IrisSectionCard {
            Text(
                text = "Owner npub",
                style = MaterialTheme.typography.titleSmall,
            )
            Text(
                text = account.npub,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.testTag("awaitingApprovalOwnerNpub"),
            )
            Text(
                text = "This device npub",
                style = MaterialTheme.typography.titleSmall,
            )
            Text(
                text = account.deviceNpub,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.testTag("awaitingApprovalDeviceNpub"),
            )
            IrisSecondaryButton(
                text = "Copy device npub",
                onClick = { clipboard.setText(AnnotatedString(account.deviceNpub)) },
                modifier = Modifier.testTag("awaitingApprovalCopyDeviceButton"),
                icon = {
                    Icon(
                        imageVector = IrisIcons.Copy,
                        contentDescription = null,
                    )
                },
            )
            IrisPrimaryButton(
                text = "Logout",
                onClick = appManager::logout,
                icon = {
                    Icon(
                        imageVector = IrisIcons.Logout,
                        contentDescription = null,
                    )
                },
            )
        }
    }
}
