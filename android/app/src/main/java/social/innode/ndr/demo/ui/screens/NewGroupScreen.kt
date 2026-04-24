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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.AppAction
import social.innode.ndr.demo.rust.AppState
import social.innode.ndr.demo.rust.ChatKind
import social.innode.ndr.demo.rust.isValidPeerInput
import social.innode.ndr.demo.rust.normalizePeerInput
import social.innode.ndr.demo.ui.components.IrisIcons
import social.innode.ndr.demo.ui.components.IrisPrimaryButton
import social.innode.ndr.demo.ui.components.IrisSectionCard
import social.innode.ndr.demo.ui.components.IrisSecondaryButton
import social.innode.ndr.demo.ui.components.IrisTopBar
import social.innode.ndr.demo.ui.components.rememberIrisClipboard
import social.innode.ndr.demo.ui.theme.IrisTheme

@Composable
fun NewGroupScreen(
    appManager: AppManager,
    appState: AppState,
) {
    val clipboard = rememberIrisClipboard()
    var name by remember { mutableStateOf("") }
    var memberInput by remember { mutableStateOf("") }
    var showScanner by remember { mutableStateOf(false) }
    var selectedOwners by remember { mutableStateOf(setOf<String>()) }
    val localOwner = appState.account?.publicKeyHex
    val normalizedInput = normalizePeerInput(memberInput)
    val existingDirectChats =
        appState.chatList.filter { it.kind == ChatKind.DIRECT && it.chatId != localOwner }
    val canCreate = name.isNotBlank() && selectedOwners.isNotEmpty() && !appState.busy.creatingGroup

    fun addOwner(ownerInput: String) {
        val normalized = normalizePeerInput(ownerInput)
        if (normalized.isBlank() || !isValidPeerInput(normalized)) {
            return
        }
        if (normalized == localOwner) {
            return
        }
        selectedOwners = selectedOwners + normalized
        memberInput = ""
    }

    ScaffoldScreen(
        title = "New group",
        appManager = appManager,
        appState = appState,
    ) {
        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .verticalScroll(rememberScrollState())
                    .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            IrisSectionCard {
                Text(
                    text = "Create a group",
                    style = MaterialTheme.typography.titleLarge,
                )
                Text(
                    text = "Pick members from existing direct chats or paste and scan owner npubs. The creator stays the managing admin in this client.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = IrisTheme.palette.muted,
                )
            }

            IrisSectionCard {
                Text(
                    text = "Group name",
                    style = MaterialTheme.typography.titleMedium,
                )
                TextField(
                    value = name,
                    onValueChange = { name = it },
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .testTag("newGroupNameInput"),
                    placeholder = {
                        Text(
                            text = "Weekend plans",
                            color = IrisTheme.palette.muted,
                        )
                    },
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
                            .testTag("newGroupMemberInput"),
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
                        onClick = {
                            clipboard.getText { text ->
                                memberInput = normalizePeerInput(text)
                            }
                        },
                        modifier = Modifier.testTag("newGroupPasteButton"),
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
                        modifier = Modifier.testTag("newGroupScanQrButton"),
                        icon = {
                            Icon(
                                imageVector = IrisIcons.ScanQr,
                                contentDescription = null,
                            )
                        },
                    )
                    IrisPrimaryButton(
                        text = "Add",
                        onClick = { addOwner(normalizedInput) },
                        enabled = normalizedInput.isNotBlank() && isValidPeerInput(normalizedInput),
                        modifier = Modifier.testTag("newGroupAddMemberButton"),
                        icon = {
                            Icon(
                                imageVector = IrisIcons.NewGroup,
                                contentDescription = null,
                            )
                        },
                    )
                }

                if (selectedOwners.isNotEmpty()) {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        selectedOwners.toList().sorted().forEach { owner ->
                            MemberChip(
                                label = normalizePeerInput(owner),
                                onRemove = { selectedOwners = selectedOwners - owner },
                            )
                        }
                    }
                }
            }

            if (existingDirectChats.isNotEmpty()) {
                IrisSectionCard {
                    Text(
                        text = "Existing chats",
                        style = MaterialTheme.typography.titleMedium,
                    )
                    existingDirectChats.forEach { chat ->
                        val selected = chat.chatId in selectedOwners
                        IrisSecondaryButton(
                            text = if (selected) "Selected: ${chat.displayName}" else chat.displayName,
                            onClick = {
                                selectedOwners =
                                    if (selected) {
                                        selectedOwners - chat.chatId
                                    } else {
                                        selectedOwners + chat.chatId
                                    }
                            },
                            modifier = Modifier.fillMaxWidth(),
                            icon = {
                                Icon(
                                    imageVector = if (selected) IrisIcons.Devices else IrisIcons.NewChat,
                                    contentDescription = null,
                                )
                            },
                        )
                    }
                }
            }

            IrisPrimaryButton(
                text = if (appState.busy.creatingGroup) "Creating…" else "Create group",
                onClick = { appManager.createGroup(name, selectedOwners.toList()) },
                enabled = canCreate,
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .testTag("newGroupCreateButton"),
                icon = {
                    Icon(
                        imageVector = IrisIcons.NewGroup,
                        contentDescription = null,
                    )
                },
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
                    addOwner(normalized)
                    showScanner = false
                    null
                }
            },
        )
    }
}

@Composable
private fun ScaffoldScreen(
    title: String,
    appManager: AppManager,
    appState: AppState,
    content: @Composable () -> Unit,
) {
    androidx.compose.material3.Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            IrisTopBar(
                title = title,
                onBack = {
                    appManager.dispatch(
                        AppAction.UpdateScreenStack(appState.router.screenStack.dropLast(1)),
                    )
                },
            )
        },
    ) { padding ->
        Surface(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding),
            color = MaterialTheme.colorScheme.background,
        ) {
            content()
        }
    }
}

@Composable
private fun MemberChip(
    label: String,
    onRemove: () -> Unit,
) {
    Surface(
        color = IrisTheme.palette.panelAlt,
        shape = androidx.compose.foundation.shape.RoundedCornerShape(100.dp),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium,
            )
            Text(
                text = "Remove",
                modifier = Modifier.testTag("memberChipRemove").clickable(onClick = onRemove),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.error,
            )
        }
    }
}
