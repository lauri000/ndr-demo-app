package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.AppAction
import social.innode.ndr.demo.rust.AppState

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ChatScreen(
    appManager: AppManager,
    appState: AppState,
    chatId: String,
) {
    val chat = appState.currentChat?.takeIf { it.chatId == chatId }
    var draft by remember(chatId) { mutableStateOf("") }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(chat?.displayName ?: "Chat") },
                navigationIcon = {
                    TextButton(
                        onClick = {
                            appManager.dispatch(
                                AppAction.UpdateScreenStack(appState.router.screenStack.dropLast(1)),
                            )
                        },
                    ) {
                        Text("Back")
                    }
                },
            )
        },
    ) { padding ->
        if (chat == null) {
            Box(
                modifier =
                    Modifier
                        .fillMaxSize()
                        .padding(padding),
                contentAlignment = Alignment.Center,
            ) {
                Text("Loading chat…")
            }
            return@Scaffold
        }

        Column(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding),
        ) {
            LazyColumn(
                modifier =
                    Modifier
                        .weight(1f)
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 8.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                items(chat.messages, key = { it.id }) { message ->
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement =
                            if (message.isOutgoing) {
                                Arrangement.End
                            } else {
                                Arrangement.Start
                            },
                    ) {
                        Surface(
                            color =
                                if (message.isOutgoing) {
                                    MaterialTheme.colorScheme.primaryContainer
                                } else {
                                    MaterialTheme.colorScheme.secondaryContainer
                                },
                            shape = MaterialTheme.shapes.large,
                        ) {
                            Column(
                                modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
                                verticalArrangement = Arrangement.spacedBy(4.dp),
                            ) {
                                Text(
                                    text = message.body,
                                    style = MaterialTheme.typography.bodyLarge,
                                )
                                Text(
                                    text = message.author,
                                    style = MaterialTheme.typography.labelSmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    textAlign = if (message.isOutgoing) TextAlign.End else TextAlign.Start,
                                )
                            }
                        }
                    }
                }
            }

            Column(
                modifier =
                    Modifier
                        .fillMaxWidth()
                        .padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                OutlinedTextField(
                    value = draft,
                    onValueChange = { draft = it },
                    label = { Text("Message") },
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .testTag("chatMessageInput"),
                    minLines = 2,
                )
                Button(
                    onClick = {
                        appManager.sendText(chatId, draft)
                        draft = ""
                    },
                    enabled = draft.isNotBlank() && !appState.busy.sendingMessage,
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .testTag("chatSendButton"),
                ) {
                    Text("Send")
                }
            }
        }
    }
}
