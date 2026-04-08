package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
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
import social.innode.ndr.demo.ui.components.IrisIcons
import social.innode.ndr.demo.ui.components.IrisInlineAction
import social.innode.ndr.demo.ui.components.IrisPrimaryButton
import social.innode.ndr.demo.ui.components.IrisSectionCard
import social.innode.ndr.demo.ui.components.IrisSecondaryButton
import social.innode.ndr.demo.ui.theme.IrisTheme

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

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        containerColor = MaterialTheme.colorScheme.background,
    ) {
        Column(
            modifier =
                Modifier
                    .testTag("myProfileSheet")
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            IrisSectionCard {
                Text(
                    text = "My profile",
                    style = MaterialTheme.typography.headlineSmall,
                )
                Text(
                    text = "Scan this owner QR from a fresh device to start linking it. The primary device still controls roster approval.",
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
                            contentDescription = "My npub QR code",
                            modifier =
                                Modifier
                                    .size(260.dp)
                                    .testTag("myProfileQrCode"),
                        )
                    }
                }
                IrisInlineAction(
                    text = "Copy owner npub",
                    onClick = { clipboard.setText(AnnotatedString(npub)) },
                ) {
                    Icon(imageVector = IrisIcons.Copy, contentDescription = null)
                }
            }

            IrisSectionCard {
                Text(
                    text = "Owner npub",
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    npub,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.testTag("myProfileNpubValue"),
                )
                Text(
                    text = "Current device npub",
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = deviceNpub,
                    style = MaterialTheme.typography.bodySmall,
                    color = IrisTheme.palette.muted,
                )
                Text(
                    text = "Public key hex",
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = publicKeyHex,
                    style = MaterialTheme.typography.bodySmall,
                    color = IrisTheme.palette.muted,
                )
            }

            if (canManageDevices) {
                IrisPrimaryButton(
                    text = "Manage devices",
                    onClick = onManageDevices,
                    modifier = Modifier.testTag("myProfileManageDevicesButton"),
                    icon = {
                        Icon(
                            imageVector = IrisIcons.Devices,
                            contentDescription = null,
                        )
                    },
                )
            }

            IrisSecondaryButton(
                text = "Logout",
                onClick = onLogout,
                modifier = Modifier.testTag("myProfileLogoutButton"),
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
