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
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Badge
import androidx.compose.material3.CenterAlignedTopAppBar
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.AppState
import social.innode.ndr.demo.rust.Screen

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChatListScreen(
    appManager: AppManager,
    appState: AppState,
) {
    var showProfile by remember { mutableStateOf(false) }
    val account = appState.account

    Scaffold(
        topBar = {
            CenterAlignedTopAppBar(
                title = { Text("Chats") },
                colors =
                    TopAppBarDefaults.centerAlignedTopAppBarColors(
                        containerColor = MaterialTheme.colorScheme.surface,
                    ),
                navigationIcon = {
                    if (account != null) {
                        Box(
                            modifier =
                                Modifier
                                    .padding(start = 12.dp)
                                    .size(36.dp)
                                    .clip(CircleShape)
                                    .background(MaterialTheme.colorScheme.secondaryContainer)
                                    .testTag("chatListProfileButton")
                                    .clickable { showProfile = true },
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                text = account.npub.take(1).uppercase(),
                                style = MaterialTheme.typography.titleSmall,
                                color = MaterialTheme.colorScheme.onSecondaryContainer,
                            )
                        }
                    }
                },
                actions = {
                    TextButton(
                        onClick = { appManager.pushScreen(Screen.NewChat) },
                        modifier = Modifier.testTag("chatListNewChatButton"),
                    ) {
                        Text("New chat")
                    }
                },
            )
        },
    ) { padding ->
        if (appState.chatList.isEmpty()) {
            Box(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .padding(padding)
                        .padding(24.dp),
                contentAlignment = Alignment.Center,
            ) {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        text = "No chats yet",
                        style = MaterialTheme.typography.headlineSmall,
                    )
                    Text(
                        text = "Start a direct chat with an npub, hex pubkey, or QR scan.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    TextButton(onClick = { appManager.pushScreen(Screen.NewChat) }) {
                        Text(
                            "Start a chat",
                            modifier = Modifier.testTag("chatListEmptyStateStartButton"),
                        )
                    }
                }
            }
        } else {
            LazyColumn(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .padding(padding),
            ) {
                items(appState.chatList, key = { it.chatId }) { chat ->
                    Row(
                        modifier =
                            Modifier
                                .fillMaxWidth()
                                .clickable { appManager.openChat(chat.chatId) }
                                .padding(horizontal = 18.dp, vertical = 14.dp)
                                .testTag("chatRow-${chat.chatId.take(12)}"),
                        horizontalArrangement = Arrangement.spacedBy(12.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Box(
                            modifier =
                                Modifier
                                    .size(40.dp)
                                    .clip(CircleShape)
                                    .background(MaterialTheme.colorScheme.secondaryContainer),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                text = chat.displayName.take(1).uppercase(),
                                color = MaterialTheme.colorScheme.onSecondaryContainer,
                                fontWeight = FontWeight.SemiBold,
                            )
                        }

                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = chat.displayName,
                                style = MaterialTheme.typography.titleMedium,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                text = chat.lastMessagePreview ?: chat.peerNpub,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }

                        if (chat.unreadCount > 0uL) {
                            Badge {
                                Text(chat.unreadCount.toString())
                            }
                        }
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
