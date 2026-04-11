package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.AppState
import social.innode.ndr.demo.rust.ChatKind
import social.innode.ndr.demo.rust.Screen
import social.innode.ndr.demo.ui.components.IrisAvatar
import social.innode.ndr.demo.ui.components.IrisChatListRow
import social.innode.ndr.demo.ui.components.IrisDivider
import social.innode.ndr.demo.ui.components.IrisIcons
import social.innode.ndr.demo.ui.components.IrisPrimaryButton
import social.innode.ndr.demo.ui.components.IrisSectionCard
import social.innode.ndr.demo.ui.components.IrisSecondaryButton
import social.innode.ndr.demo.ui.components.IrisTopBar
import social.innode.ndr.demo.ui.components.formatRelativeTime
import social.innode.ndr.demo.ui.theme.IrisTheme

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChatListScreen(
    appManager: AppManager,
    appState: AppState,
) {
    var showProfile by remember { mutableStateOf(false) }
    var showNewChooser by remember { mutableStateOf(false) }
    val account = appState.account

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            IrisTopBar(
                title = "Chats",
                leading = {
                    if (account != null) {
                        Box(
                            modifier =
                                Modifier
                                    .padding(start = 4.dp)
                                    .testTag("chatListProfileButton")
                                    .clickable { showProfile = true },
                        ) {
                            IrisAvatar(label = account.displayName, emphasize = true, size = 44.dp)
                        }
                    }
                },
                actions = {
                    IrisPrimaryButton(
                        text = "New",
                        onClick = { showNewChooser = true },
                        modifier = Modifier.testTag("chatListNewChatButton"),
                        icon = {
                            Icon(
                                imageVector = IrisIcons.NewChat,
                                contentDescription = null,
                            )
                        },
                    )
                },
            )
        },
    ) { padding ->
        LazyColumn(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .background(MaterialTheme.colorScheme.background),
        ) {
            item {
                IrisSectionCard(
                    modifier =
                        Modifier
                            .padding(horizontal = 16.dp, vertical = 14.dp)
                            .testTag("chatListNewChatCard"),
                ) {
                    Text(
                        text = "Conversations",
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = "Direct chats and groups live together here. Start a 1:1 conversation with an npub or create a group and manage it from the thread itself.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = IrisTheme.palette.muted,
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        IrisPrimaryButton(
                            text = "New chat",
                            onClick = { appManager.pushScreen(Screen.NewChat) },
                            modifier = Modifier.testTag("chatListEmptyStateStartButton"),
                            icon = {
                                Icon(
                                    imageVector = IrisIcons.NewChat,
                                    contentDescription = null,
                                )
                            },
                        )
                        IrisSecondaryButton(
                            text = "New group",
                            onClick = { appManager.pushScreen(Screen.NewGroup) },
                            modifier = Modifier.testTag("chatListEmptyStateGroupButton"),
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

            if (appState.chatList.isEmpty()) {
                item {
                    Box(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 16.dp, vertical = 12.dp),
                        contentAlignment = Alignment.Center,
                    ) {
                        Text(
                            text = "No chats yet",
                            style = MaterialTheme.typography.bodyLarge,
                            color = IrisTheme.palette.muted,
                        )
                    }
                }
            } else {
                items(appState.chatList, key = { it.chatId }) { chat ->
                    val subtitle = chat.subtitle
                    Column(modifier = Modifier.fillMaxWidth()) {
                        IrisChatListRow(
                            title = chat.displayName,
                            preview = chat.lastMessagePreview ?: subtitle.orEmpty(),
                            timeLabel = formatRelativeTime(chat.lastMessageAtSecs?.toLong()),
                            unreadCount = chat.unreadCount.toLong(),
                            lastMessageMine = chat.lastMessageIsOutgoing == true,
                            lastDelivery = chat.lastMessageDelivery,
                            onClick = { appManager.openChat(chat.chatId) },
                            modifier = Modifier.testTag("chatRow-${chat.chatId.take(12)}"),
                        )
                        if (chat.kind == ChatKind.GROUP && subtitle != null) {
                            Text(
                                text = subtitle,
                                modifier = Modifier.padding(start = 70.dp, bottom = 10.dp),
                                style = MaterialTheme.typography.labelMedium,
                                color = IrisTheme.palette.muted,
                            )
                        }
                        IrisDivider(modifier = Modifier.padding(start = 70.dp))
                    }
                }
            }
        }
    }

    if (showProfile && account != null) {
        MyProfileSheet(
            appManager = appManager,
            npub = account.npub,
            displayName = account.displayName,
            publicKeyHex = account.publicKeyHex,
            deviceNpub = account.deviceNpub,
            canManageDevices = account.hasOwnerSigningAuthority,
            onManageDevices = {
                showProfile = false
                appManager.pushScreen(Screen.DeviceRoster)
            },
            onLogout = {
                showProfile = false
                appManager.logout()
            },
            onDismiss = { showProfile = false },
        )
    }

    if (showNewChooser) {
        ModalBottomSheet(
            onDismissRequest = { showNewChooser = false },
            containerColor = MaterialTheme.colorScheme.background,
        ) {
            Column(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                IrisSectionCard {
                    Text(
                        text = "Start something new",
                        style = MaterialTheme.typography.headlineSmall,
                    )
                    Text(
                        text = "Choose between a direct chat and a group. Both land in the same conversation list.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = IrisTheme.palette.muted,
                    )
                    IrisPrimaryButton(
                        text = "New chat",
                        onClick = {
                            showNewChooser = false
                            appManager.pushScreen(Screen.NewChat)
                        },
                        modifier = Modifier.testTag("chatListNewChatOption"),
                        icon = {
                            Icon(
                                imageVector = IrisIcons.NewChat,
                                contentDescription = null,
                            )
                        },
                    )
                    IrisSecondaryButton(
                        text = "New group",
                        onClick = {
                            showNewChooser = false
                            appManager.pushScreen(Screen.NewGroup)
                        },
                        modifier = Modifier.testTag("chatListNewGroupOption"),
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
}
