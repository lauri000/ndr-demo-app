package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.AppAction
import social.innode.ndr.demo.rust.AppState
import social.innode.ndr.demo.rust.ChatMessageSnapshot
import social.innode.ndr.demo.ui.components.DeliveryGlyph
import social.innode.ndr.demo.ui.components.IrisIcons
import social.innode.ndr.demo.ui.components.IrisTopBar
import social.innode.ndr.demo.ui.components.formatMessageClock
import social.innode.ndr.demo.ui.components.formatTimelineDay
import social.innode.ndr.demo.ui.components.isSameTimelineDay
import social.innode.ndr.demo.ui.components.messageBubbleShape
import social.innode.ndr.demo.ui.theme.IrisTheme

@Composable
fun ChatScreen(
    appManager: AppManager,
    appState: AppState,
    chatId: String,
) {
    val chat = appState.currentChat?.takeIf { it.chatId == chatId }
    var draft by remember(chatId) { mutableStateOf("") }
    val listState = rememberLazyListState()
    val coroutineScope = rememberCoroutineScope()
    val showJumpToBottom by remember(chat?.messages?.size, listState) {
        derivedStateOf {
            val total = chat?.messages?.size ?: 0
            total > 4 && listState.firstVisibleItemIndex < total - 4
        }
    }

    LaunchedEffect(chatId, chat?.messages?.size) {
        val total = chat?.messages?.size ?: 0
        if (total > 0) {
            listState.scrollToItem(total - 1)
        }
    }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            IrisTopBar(
                title = chat?.displayName ?: "Chat",
                onBack = {
                    appManager.dispatch(
                        AppAction.UpdateScreenStack(appState.router.screenStack.dropLast(1)),
                    )
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

        Box(
            modifier =
                Modifier
                    .fillMaxSize()
                    .padding(padding)
                    .background(MaterialTheme.colorScheme.background),
        ) {
            Column(modifier = Modifier.fillMaxSize()) {
                LazyColumn(
                    state = listState,
                    modifier =
                        Modifier
                            .weight(1f)
                            .fillMaxWidth()
                            .padding(horizontal = 14.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    itemsIndexed(chat.messages, key = { _, message -> message.id }) { index, message ->
                        val previous = chat.messages.getOrNull(index - 1)
                        val next = chat.messages.getOrNull(index + 1)
                        val showDayChip =
                            previous == null ||
                                !isSameTimelineDay(
                                    previous.createdAtSecs.toLong(),
                                    message.createdAtSecs.toLong(),
                                )
                        val isFirstInCluster =
                            previous == null || previous.isOutgoing != message.isOutgoing
                        val isLastInCluster =
                            next == null || next.isOutgoing != message.isOutgoing

                        if (showDayChip) {
                            Box(
                                modifier =
                                    Modifier
                                        .fillMaxWidth()
                                        .padding(vertical = 14.dp),
                                contentAlignment = Alignment.Center,
                            ) {
                                Surface(
                                    color = IrisTheme.palette.panel.copy(alpha = 0.85f),
                                    shape = RoundedCornerShape(100.dp),
                                ) {
                                    Text(
                                        text = formatTimelineDay(message.createdAtSecs.toLong()),
                                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                                        style = MaterialTheme.typography.labelMedium,
                                        color = IrisTheme.palette.muted,
                                    )
                                }
                            }
                        }

                        MessageBubble(
                            message = message,
                            isFirstInCluster = isFirstInCluster,
                            isLastInCluster = isLastInCluster,
                        )
                    }
                }

                ComposerBar(
                    draft = draft,
                    isSending = appState.busy.sendingMessage,
                    onDraftChange = { draft = it },
                    onSend = {
                        appManager.sendText(chatId, draft)
                        draft = ""
                    },
                )
            }

            if (showJumpToBottom) {
                Surface(
                    modifier =
                        Modifier
                            .align(Alignment.BottomEnd)
                            .padding(end = 18.dp, bottom = 104.dp)
                            .testTag("chatJumpToBottom"),
                    color = IrisTheme.palette.panel,
                    shape = CircleShape,
                    shadowElevation = 0.dp,
                ) {
                    IconButton(
                        onClick = {
                            coroutineScope.launch {
                                val total = chat.messages.size
                                if (total > 0) {
                                    listState.animateScrollToItem(total - 1)
                                }
                            }
                        },
                    ) {
                        Text(
                            text = "↓",
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Bold,
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun MessageBubble(
    message: ChatMessageSnapshot,
    isFirstInCluster: Boolean,
    isLastInCluster: Boolean,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = if (message.isOutgoing) Arrangement.End else Arrangement.Start,
    ) {
        Surface(
            color =
                if (message.isOutgoing) {
                    IrisTheme.palette.bubbleMine
                } else {
                    IrisTheme.palette.bubbleTheirs
                },
            shape =
                messageBubbleShape(
                    isOutgoing = message.isOutgoing,
                    isFirstInCluster = isFirstInCluster,
                    isLastInCluster = isLastInCluster,
                ),
            tonalElevation = 0.dp,
            shadowElevation = 0.dp,
        ) {
            Column(
                modifier =
                    Modifier
                        .padding(horizontal = 14.dp, vertical = 10.dp)
                        .testTag("chatMessage-${message.id}"),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Text(
                    text = message.body,
                    style = MaterialTheme.typography.bodyLarge,
                    color =
                        if (message.isOutgoing) {
                            MaterialTheme.colorScheme.onPrimary
                        } else {
                            MaterialTheme.colorScheme.onSurface
                        },
                )
                Row(
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = formatMessageClock(message.createdAtSecs.toLong()),
                        style = MaterialTheme.typography.labelSmall,
                        color =
                            if (message.isOutgoing) {
                                MaterialTheme.colorScheme.onPrimary.copy(alpha = 0.72f)
                            } else {
                                IrisTheme.palette.muted
                            },
                    )
                    if (message.isOutgoing) {
                        DeliveryGlyph(message.delivery)
                    }
                }
            }
        }
    }
}

@Composable
private fun ComposerBar(
    draft: String,
    isSending: Boolean,
    onDraftChange: (String) -> Unit,
    onSend: () -> Unit,
) {
    Surface(
        modifier =
            Modifier
                .fillMaxWidth()
                .imePadding(),
        color = IrisTheme.palette.toolbar,
        tonalElevation = 0.dp,
        shadowElevation = 0.dp,
    ) {
        Row(
            modifier =
                Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 14.dp, vertical = 12.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.Bottom,
        ) {
            Surface(
                modifier = Modifier.weight(1f),
                color = IrisTheme.palette.panel,
                shape = RoundedCornerShape(24.dp),
            ) {
                TextField(
                    value = draft,
                    onValueChange = onDraftChange,
                    placeholder = {
                        Text(
                            text = "Message",
                            color = IrisTheme.palette.muted,
                        )
                    },
                    modifier =
                        Modifier
                            .fillMaxWidth()
                            .testTag("chatMessageInput"),
                    minLines = 1,
                    maxLines = 5,
                    colors =
                        TextFieldDefaults.colors(
                            focusedContainerColor = Color.Transparent,
                            unfocusedContainerColor = Color.Transparent,
                            disabledContainerColor = Color.Transparent,
                            focusedIndicatorColor = Color.Transparent,
                            unfocusedIndicatorColor = Color.Transparent,
                            disabledIndicatorColor = Color.Transparent,
                        ),
                )
            }

            Surface(
                modifier =
                    Modifier
                        .size(52.dp)
                        .clip(CircleShape),
                color = IrisTheme.palette.accent,
                shape = CircleShape,
            ) {
                IconButton(
                    onClick = onSend,
                    enabled = draft.isNotBlank() && !isSending,
                    modifier = Modifier.testTag("chatSendButton"),
                ) {
                    if (isSending) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(20.dp),
                            strokeWidth = 2.dp,
                            color = MaterialTheme.colorScheme.onPrimary,
                        )
                    } else {
                        Icon(
                            imageVector = IrisIcons.Send,
                            contentDescription = "Send",
                            tint = MaterialTheme.colorScheme.onPrimary,
                        )
                    }
                }
            }
        }
    }
}
