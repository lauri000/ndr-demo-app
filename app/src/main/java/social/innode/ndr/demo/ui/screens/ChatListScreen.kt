package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
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
import social.innode.ndr.demo.rust.Screen
import social.innode.ndr.demo.ui.components.IrisAvatar
import social.innode.ndr.demo.ui.components.IrisChatListRow
import social.innode.ndr.demo.ui.components.IrisDivider
import social.innode.ndr.demo.ui.components.IrisIcons
import social.innode.ndr.demo.ui.components.IrisPrimaryButton
import social.innode.ndr.demo.ui.components.IrisSectionCard
import social.innode.ndr.demo.ui.components.IrisTopBar
import social.innode.ndr.demo.ui.components.formatRelativeTime
import social.innode.ndr.demo.ui.theme.IrisTheme

@Composable
fun ChatListScreen(
    appManager: AppManager,
    appState: AppState,
) {
    var showProfile by remember { mutableStateOf(false) }
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
                            IrisAvatar(label = account.npub, emphasize = true, size = 44.dp)
                        }
                    }
                },
                actions = {
                    IrisPrimaryButton(
                        text = "New",
                        onClick = { appManager.pushScreen(Screen.NewChat) },
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
                        text = "Direct messages",
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = "Start a chat with an npub, a hex key, or a QR scan. The list below keeps the latest conversation first, just like Iris web.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = IrisTheme.palette.muted,
                    )
                    IrisPrimaryButton(
                        text = "Start a chat",
                        onClick = { appManager.pushScreen(Screen.NewChat) },
                        modifier = Modifier.testTag("chatListEmptyStateStartButton"),
                        icon = {
                            Icon(
                                imageVector = IrisIcons.NewChat,
                                contentDescription = null,
                            )
                        },
                    )
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
                    Column(modifier = Modifier.fillMaxWidth()) {
                        IrisChatListRow(
                            title = chat.displayName,
                            preview = chat.lastMessagePreview ?: chat.peerNpub,
                            timeLabel = formatRelativeTime(chat.lastMessageAtSecs?.toLong()),
                            unreadCount = chat.unreadCount.toLong(),
                            lastMessageMine = chat.lastMessageIsOutgoing == true,
                            lastDelivery = chat.lastMessageDelivery,
                            onClick = { appManager.openChat(chat.chatId) },
                            modifier = Modifier.testTag("chatRow-${chat.chatId.take(12)}"),
                        )
                        IrisDivider(modifier = Modifier.padding(start = 70.dp))
                    }
                }
            }
        }
    }

    if (showProfile && account != null) {
        MyProfileSheet(
            npub = account.npub,
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
}
