package social.innode.ndr.demo.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp

@Composable
fun WelcomeScreen(
    uiState: WelcomeUiState,
    onImportValueChanged: (String) -> Unit,
    onGenerateClick: () -> Unit,
    onImportClick: () -> Unit,
    onLoggedIn: () -> Unit,
) {
    LaunchedEffect(uiState.didLogin) {
        if (uiState.didLogin) {
            onLoggedIn()
        }
    }

    Column(
        modifier =
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
        Text(
            text = "Device-to-device bootstrap",
            style = MaterialTheme.typography.headlineMedium,
        )
        Text(
            text = "Generate a fresh keypair or import an existing nsec. The Rust app core owns account creation, relay connections, persistence, and protocol state. Android only renders UI and stores the encrypted nsec.",
            style = MaterialTheme.typography.bodyLarge,
        )

        Button(
            onClick = onGenerateClick,
            enabled = !uiState.isWorking,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("generateKeyButton"),
            contentPadding = PaddingValues(vertical = 16.dp),
        ) {
            if (uiState.isWorking) {
                CircularProgressIndicator(strokeWidth = 2.dp)
            } else {
                Text("Generate new key")
            }
        }

        OutlinedTextField(
            value = uiState.importValue,
            onValueChange = onImportValueChanged,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("importKeyField"),
            label = { Text("nsec or hex private key") },
            minLines = 3,
            enabled = !uiState.isWorking,
        )

        OutlinedButton(
            onClick = onImportClick,
            enabled = !uiState.isWorking,
            modifier =
                Modifier
                    .fillMaxWidth()
                    .testTag("importKeyButton"),
            contentPadding = PaddingValues(vertical = 16.dp),
        ) {
            Text("Import existing key")
        }

        uiState.errorMessage?.let { error ->
            Text(
                text = error,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}
