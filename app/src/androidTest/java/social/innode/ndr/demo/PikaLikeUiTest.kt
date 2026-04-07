package social.innode.ndr.demo

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.test.espresso.Espresso.pressBack
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import social.innode.ndr.demo.ui.screens.QrScannerTestOverrides

@RunWith(AndroidJUnit4::class)
class PikaLikeUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun generate_account_and_open_profile_sheet() {
        composeRule.ensureChatList()
        composeRule.onNodeWithTag("chatListProfileButton", useUnmergedTree = true).performClick()

        composeRule.waitForTag("myProfileSheet")
        composeRule.onNodeWithTag("myProfileQrCode", useUnmergedTree = true).assertIsDisplayed()
    }

    @Test
    fun create_chat_and_send_message_locally() {
        composeRule.ensureChatList()
        composeRule.onNodeWithTag("chatListNewChatButton", useUnmergedTree = true).performClick()

        composeRule.waitForTag("newChatPeerInput")
        composeRule.onNodeWithTag("newChatScanQrButton", useUnmergedTree = true).assertIsDisplayed()
        composeRule.onNodeWithTag("newChatPeerInput", useUnmergedTree = true)
            .performTextInput(VALID_PEER_NPUB)
        composeRule.onNodeWithTag("newChatStartButton", useUnmergedTree = true).assertIsEnabled()
        composeRule.onNodeWithTag("newChatStartButton", useUnmergedTree = true).performClick()

        composeRule.waitForTag("chatMessageInput")
        composeRule.onNodeWithTag("chatMessageInput", useUnmergedTree = true)
            .performTextInput("hello from test")
        composeRule.onNodeWithTag("chatSendButton", useUnmergedTree = true).assertIsEnabled()
        composeRule.onNodeWithTag("chatSendButton", useUnmergedTree = true).performClick()

        composeRule.waitForText("hello from test")
        composeRule.onNodeWithTag("chatSendButton", useUnmergedTree = true).assertIsNotEnabled()
    }

    @Test
    fun scan_qr_populates_new_chat_input() {
        composeRule.ensureChatList()
        composeRule.onNodeWithTag("chatListNewChatButton", useUnmergedTree = true).performClick()

        composeRule.waitForTag("newChatPeerInput")
        composeRule.runOnUiThread {
            QrScannerTestOverrides.nextScannedValue = VALID_PEER_NPUB
        }
        composeRule.onNodeWithTag("newChatScanQrButton", useUnmergedTree = true).performClick()

        composeRule.waitUntil(5_000) {
            composeRule
                .onAllNodesWithTag("newChatPeerInput", useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
        composeRule
            .onNodeWithTag("newChatPeerInput", useUnmergedTree = true)
            .assertTextContains(VALID_PEER_NPUB)
        composeRule.onNodeWithTag("newChatStartButton", useUnmergedTree = true).assertIsEnabled()
    }

    private fun androidx.compose.ui.test.junit4.AndroidComposeTestRule<*, *>.waitForTag(
        tag: String,
        timeoutMillis: Long = 15_000,
    ) {
        waitUntil(timeoutMillis) {
            onAllNodesWithTag(tag, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()
        }
    }

    private fun androidx.compose.ui.test.junit4.AndroidComposeTestRule<*, *>.waitForText(
        text: String,
        timeoutMillis: Long = 15_000,
    ) {
        waitUntil(timeoutMillis) {
            onAllNodesWithText(text, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()
        }
    }

    companion object {
        private const val VALID_PEER_NPUB =
            "npub18w35g6gn47qwmryulxzvfucmujvrqqljjpapyl8x0rqaljh6f2usml77dj"
    }

    private fun androidx.compose.ui.test.junit4.AndroidComposeTestRule<*, *>.ensureChatList() {
        waitUntil(30_000) {
            hasTag("generateKeyButton") ||
                hasTag("chatListNewChatButton") ||
                hasTag("newChatPeerInput") ||
                hasTag("chatMessageInput") ||
                hasTag("myProfileSheet")
        }

        if (hasTag("generateKeyButton")) {
            onNodeWithTag("generateKeyButton", useUnmergedTree = true).performClick()
            waitForTag("chatListNewChatButton")
            return
        }

        repeat(3) {
            if (hasTag("chatListNewChatButton")) {
                return
            }
            if (hasTag("newChatPeerInput") || hasTag("chatMessageInput") || hasTag("myProfileSheet")) {
                pressBack()
                waitForIdle()
            }
        }

        waitForTag("chatListNewChatButton")
    }

    private fun androidx.compose.ui.test.junit4.AndroidComposeTestRule<*, *>.hasTag(tag: String): Boolean =
        onAllNodesWithTag(tag, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()
}
