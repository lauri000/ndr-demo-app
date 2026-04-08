package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.unit.dp

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MyProfileSheet(
    npub: String,
    publicKeyHex: String,
    deviceNpub: String,
    canManageDevices: Boolean,
    onManageDevices: () -> Unit,
    onLogout: () -> Unit,
    onDismiss: () -> Unit,
) {
    val clipboard = LocalClipboardManager.current
    val qrBitmap =
        remember(npub) {
            createQrBitmap(npub, size = 768)
        }

    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(
            modifier =
                Modifier
                    .testTag("myProfileSheet")
                    .fillMaxWidth()
                    .padding(horizontal = 24.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                text = "My profile",
                style = MaterialTheme.typography.headlineSmall,
            )

            Box(
                modifier = Modifier.fillMaxWidth(),
                contentAlignment = Alignment.Center,
            ) {
                if (qrBitmap != null) {
                    Image(
                        bitmap = qrBitmap.asImageBitmap(),
                        contentDescription = "My npub QR code",
                        modifier =
                            Modifier
                                .size(260.dp)
                                .testTag("myProfileQrCode"),
                    )
                }
            }

            Text("npub", style = MaterialTheme.typography.titleSmall)
            Text(
                npub,
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.testTag("myProfileNpubValue"),
            )

            Text("Public key hex", style = MaterialTheme.typography.titleSmall)
            Text(publicKeyHex, style = MaterialTheme.typography.bodySmall)

            Text("Current device npub", style = MaterialTheme.typography.titleSmall)
            Text(deviceNpub, style = MaterialTheme.typography.bodySmall)

            TextButton(onClick = { clipboard.setText(AnnotatedString(npub)) }) {
                Text("Copy npub")
            }

            if (canManageDevices) {
                TextButton(
                    onClick = onManageDevices,
                    modifier = Modifier.testTag("myProfileManageDevicesButton"),
                ) {
                    Text("Manage devices")
                }
            }

            TextButton(
                onClick = onLogout,
                modifier = Modifier.testTag("myProfileLogoutButton"),
            ) {
                Text("Logout")
            }
        }
    }
}
