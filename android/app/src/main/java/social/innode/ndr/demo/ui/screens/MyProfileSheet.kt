package social.innode.ndr.demo.ui.screens

import android.content.Intent
import android.net.Uri
import android.widget.Toast
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.ui.components.IrisIcons
import social.innode.ndr.demo.ui.components.IrisInlineAction
import social.innode.ndr.demo.ui.components.IrisPrimaryButton
import social.innode.ndr.demo.ui.components.IrisSectionCard
import social.innode.ndr.demo.ui.components.IrisSecondaryButton
import social.innode.ndr.demo.ui.components.rememberIrisClipboard
import social.innode.ndr.demo.ui.theme.IrisTheme

private const val IrisSourceUrl =
    "https://git.iris.to/#/npub1xdhnr9mrv47kkrn95k6cwecearydeh8e895990n3acntwvmgk2dsdeeycm/iris-chat-rs"
private const val IrisSourceLabel = "git.iris.to/iris-chat-rs"

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun MyProfileSheet(
    appManager: AppManager,
    npub: String,
    displayName: String,
    publicKeyHex: String,
    deviceNpub: String,
    canManageDevices: Boolean,
    onManageDevices: () -> Unit,
    onLogout: () -> Unit,
    onDismiss: () -> Unit,
) {
    val clipboard = rememberIrisClipboard()
    val context = LocalContext.current
    val coroutineScope = rememberCoroutineScope()
    val qrBitmap =
        remember(npub) {
            createQrBitmap(npub, size = 768)
        }
    var supportBusy by remember { mutableStateOf(false) }

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
                    text = displayName,
                    style = MaterialTheme.typography.headlineSmall,
                )
                Text(
                    text = "My profile",
                    style = MaterialTheme.typography.titleMedium,
                    color = IrisTheme.palette.muted,
                )
                Text(
                    text = "Scan this owner QR from a fresh device to start linking it. The primary device still controls roster approval.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = IrisTheme.palette.muted,
                )
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
                    onClick = { clipboard.setText("Owner npub", npub) },
                ) {
                    Icon(imageVector = IrisIcons.Copy, contentDescription = null)
                }
            }

            IrisSectionCard {
                Text(
                    text = "About",
                    style = MaterialTheme.typography.titleMedium,
                )
                Text(
                    text = "Version",
                    style = MaterialTheme.typography.titleSmall,
                )
                Text(
                    text = appManager.buildSummary(),
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.testTag("myProfileVersionValue"),
                )
                IrisInlineAction(
                    text = "Source code",
                    onClick = {
                        context.startActivity(
                            Intent(Intent.ACTION_VIEW, Uri.parse(IrisSourceUrl)),
                        )
                    },
                    modifier = Modifier.testTag("myProfileSourceCodeButton"),
                ) {
                    Icon(imageVector = IrisIcons.File, contentDescription = null)
                }
                Text(
                    text = IrisSourceLabel,
                    style = MaterialTheme.typography.bodySmall,
                    color = IrisTheme.palette.muted,
                    modifier = Modifier.testTag("myProfileSourceCodeValue"),
                )
            }

            if (appManager.isTrustedTestBuild()) {
                IrisSectionCard {
                    Text(
                        text = "Trusted test build",
                        style = MaterialTheme.typography.titleMedium,
                    )
                    Text(
                        text = "This beta uses a controlled relay set and is not for sensitive conversations. Expect occasional resets and export a support bundle before reporting issues.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = IrisTheme.palette.muted,
                    )
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

            IrisSectionCard {
                Text(
                    text = "Support",
                    style = MaterialTheme.typography.titleMedium,
                )
                Text(
                    text = "Build ${appManager.buildSummary()}",
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    text = "Relay set ${appManager.relaySetId()}",
                    style = MaterialTheme.typography.bodySmall,
                    color = IrisTheme.palette.muted,
                )
                IrisPrimaryButton(
                    text = if (supportBusy) "Preparing…" else "Share support bundle",
                    onClick = {
                        coroutineScope.launch {
                            supportBusy = true
                            val bundle = appManager.exportSupportBundleJson()
                            supportBusy = false
                            val intent =
                                Intent(Intent.ACTION_SEND).apply {
                                    type = "application/json"
                                    putExtra(Intent.EXTRA_SUBJECT, "Iris Chat support bundle")
                                    putExtra(Intent.EXTRA_TEXT, bundle)
                                }
                            context.startActivity(
                                Intent.createChooser(intent, "Share support bundle"),
                            )
                        }
                    },
                    enabled = !supportBusy,
                    modifier = Modifier.testTag("myProfileShareSupportBundleButton"),
                    icon = {
                        Icon(
                            imageVector = IrisIcons.Copy,
                            contentDescription = null,
                        )
                    },
                )
                IrisSecondaryButton(
                    text = "Copy support bundle",
                    onClick = {
                        coroutineScope.launch {
                            supportBusy = true
                            val bundle = appManager.exportSupportBundleJson()
                            supportBusy = false
                            clipboard.setText("Support bundle", bundle)
                            Toast.makeText(context, "Support bundle copied", Toast.LENGTH_SHORT).show()
                        }
                    },
                    enabled = !supportBusy,
                    modifier = Modifier.testTag("myProfileCopySupportBundleButton"),
                )
                IrisSecondaryButton(
                    text = "Reset app state",
                    onClick = {
                        onDismiss()
                        appManager.resetAppState()
                    },
                    modifier = Modifier.testTag("myProfileResetStateButton"),
                    icon = {
                        Icon(
                            imageVector = IrisIcons.Logout,
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
