package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp

@Composable
fun DummyChatScreen(
    uiState: DummyChatUiState,
    onPeerChanged: (String) -> Unit,
    onDraftChanged: (String) -> Unit,
    onSendClick: () -> Unit,
    onBack: () -> Unit,
) {
    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text("Relay Rust chat", style = MaterialTheme.typography.headlineSmall)
            TextButton(onClick = onBack) {
                Text("Back")
            }
        }

        Text(
            text = "Enter the other phone's npub on both devices. The Rust app core publishes your local invite and roster, subscribes to that peer on public relays, and processes the resulting protocol events.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )

        OutlinedTextField(
            value = uiState.peerNpub,
            onValueChange = onPeerChanged,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("peerNpubField"),
            label = { Text("Peer npub or label") },
        )

        uiState.errorMessage?.let { error ->
            Text(
                text = error,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyMedium,
            )
        }

        LazyColumn(
            modifier =
                Modifier
                    .weight(1f)
                    .fillMaxWidth(),
            verticalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            items(uiState.messages, key = { it.id }) { message ->
                Surface(
                    color =
                        if (message.isOutgoing) {
                            MaterialTheme.colorScheme.primaryContainer
                        } else {
                            MaterialTheme.colorScheme.secondaryContainer
                        },
                    tonalElevation = 1.dp,
                    shape = MaterialTheme.shapes.medium,
                ) {
                    Text(
                        text = message.text,
                        modifier = Modifier.padding(12.dp),
                        style = MaterialTheme.typography.bodyLarge,
                    )
                }
            }
        }

        OutlinedTextField(
            value = uiState.draft,
            onValueChange = onDraftChanged,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("dummyMessageField"),
            label = { Text("Message") },
            minLines = 2,
        )

        Button(
            onClick = onSendClick,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("sendDummyMessageButton"),
        ) {
            Text("Send")
        }
    }
}
