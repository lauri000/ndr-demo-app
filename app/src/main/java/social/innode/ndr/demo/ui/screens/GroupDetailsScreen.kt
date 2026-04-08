package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.clickable
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
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
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
fun GroupDetailsScreen(
    appManager: AppManager,
    appState: AppState,
    groupId: String,
) {
    val details = appState.groupDetails?.takeIf { it.groupId == groupId }
    val clipboard = LocalClipboardManager.current
    var renameValue by remember(groupId, details?.name) { mutableStateOf(details?.name.orEmpty()) }
    var memberInput by remember(groupId) { mutableStateOf("") }
    var showScanner by remember { mutableStateOf(false) }
    val normalizedInput = normalizePeerInput(memberInput)

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            IrisTopBar(
                title = details?.name ?: "Group details",
                onBack = {
                    appManager.dispatch(
                        AppAction.UpdateScreenStack(appState.router.screenStack.dropLast(1)),
                    )
                },
            )
        },
    ) { padding ->
        if (details == null) {
            Surface(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .padding(padding),
                color = MaterialTheme.colorScheme.background,
            ) {
                Text(
                    text = "Loading group…",
                    modifier = Modifier.padding(24.dp),
                )
            }
            return@Scaffold
        }

        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp, vertical = 14.dp)
                    .testTag("groupDetailsScreen"),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            IrisSectionCard {
                Text(
                    text = details.name,
                    style = MaterialTheme.typography.headlineSmall,
                )
                Text(
                    text = "${details.members.size} members · revision ${details.revision}",
                    style = MaterialTheme.typography.bodyMedium,
                    color = IrisTheme.palette.muted,
                )
                Text(
                    text = "Created by ${details.createdByNpub}",
                    style = MaterialTheme.typography.bodySmall,
                    color = IrisTheme.palette.muted,
                )
            }

            IrisSectionCard {
                Text(
                    text = "Members",
                    style = MaterialTheme.typography.titleMedium,
                )
                details.members.forEach { member ->
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Column(
                            modifier = Modifier.weight(1f),
                            verticalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            Text(
                                text = member.npub,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                            Text(
                                text =
                                    buildString {
                                        if (member.isCreator) append("Creator")
                                        if (member.isAdmin) {
                                            if (isNotEmpty()) append(" · ")
                                            append("Admin")
                                        }
                                        if (member.isLocalOwner) {
                                            if (isNotEmpty()) append(" · ")
                                            append("This device owner")
                                        }
                                    },
                                style = MaterialTheme.typography.bodySmall,
                                color = IrisTheme.palette.muted,
                            )
                        }
                        if (details.canManage && !member.isLocalOwner) {
                            Text(
                                text = "Remove",
                                modifier =
                                    Modifier
                                        .testTag("groupDetailsRemoveMember-${member.ownerPubkeyHex.take(12)}")
                                        .clickable {
                                            appManager.removeGroupMember(groupId, member.ownerPubkeyHex)
                                        },
                                color = MaterialTheme.colorScheme.error,
                                style = MaterialTheme.typography.labelLarge,
                            )
                        }
                    }
                }
            }

            if (details.canManage) {
                IrisSectionCard {
                    Text(
                        text = "Rename group",
                        style = MaterialTheme.typography.titleMedium,
                    )
                    TextField(
                        value = renameValue,
                        onValueChange = { renameValue = it },
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .testTag("groupDetailsNameInput"),
                        singleLine = true,
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
                    IrisPrimaryButton(
                        text = if (appState.busy.updatingGroup) "Saving…" else "Save name",
                        onClick = { appManager.updateGroupName(groupId, renameValue) },
                        enabled = renameValue.isNotBlank() && !appState.busy.updatingGroup,
                        modifier = Modifier.testTag("groupDetailsRenameButton"),
                        icon = {
                            Icon(
                                imageVector = IrisIcons.Edit,
                                contentDescription = null,
                            )
                        },
                    )
                }

                IrisSectionCard {
                    Text(
                        text = "Add members",
                        style = MaterialTheme.typography.titleMedium,
                    )
                    TextField(
                        value = memberInput,
                        onValueChange = { memberInput = it },
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .testTag("groupDetailsAddMemberInput"),
                        placeholder = {
                            Text(
                                text = "npub, hex, or nostr:...",
                                color = IrisTheme.palette.muted,
                            )
                        },
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
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        IrisSecondaryButton(
                            text = "Paste",
                            onClick = { memberInput = normalizePeerInput(clipboard.getText()?.text.orEmpty()) },
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
                            modifier = Modifier.testTag("groupDetailsScanQrButton"),
                            icon = {
                                Icon(
                                    imageVector = IrisIcons.ScanQr,
                                    contentDescription = null,
                                )
                            },
                        )
                    }
                    IrisPrimaryButton(
                        text = if (appState.busy.updatingGroup) "Adding…" else "Add member",
                        onClick = {
                            appManager.addGroupMembers(groupId, listOf(normalizedInput))
                            memberInput = ""
                        },
                        enabled = normalizedInput.isNotBlank() && isValidPeerInput(normalizedInput) && !appState.busy.updatingGroup,
                        modifier = Modifier.testTag("groupDetailsAddMembersButton"),
                        icon = {
                            Icon(
                                imageVector = IrisIcons.NewGroup,
                                contentDescription = null,
                            )
                        },
                    )
                }
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
                    memberInput = normalized
                    showScanner = false
                    null
                }
            },
        )
    }
}
