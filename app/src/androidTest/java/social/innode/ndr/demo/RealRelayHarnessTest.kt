package social.innode.ndr.demo

import android.os.Bundle
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.test.espresso.Espresso.pressBack
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.rust.CurrentChatSnapshot
import social.innode.ndr.demo.rust.DeliveryState
import social.innode.ndr.demo.rust.normalizePeerInput

@RunWith(AndroidJUnit4::class)
class RealRelayHarnessTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    private val instrumentation
        get() = InstrumentationRegistry.getInstrumentation()

    private val arguments
        get() = InstrumentationRegistry.getArguments()

    private fun appManager(): AppManager =
        (composeRule.activity.application as NdrDemoApp).container.appManager

    @Test
    fun create_account_and_report_identity() {
        ensureChatList()
        val account = waitForState("account") { appManager().state.value.account }
        reportStatus(
            "npub" to account.npub,
            "public_key_hex" to account.publicKeyHex,
            "app_package" to composeRule.activity.packageName,
            "data_dir" to composeRule.activity.filesDir.absolutePath,
        )
    }

    @Test
    fun create_chat_from_args() {
        ensureChatList()
        val peerInput = requiredArg("peer_input")
        val chat = ensureChatOpen(peerInput)
        reportStatus(
            "chat_id" to chat.chatId,
            "peer_npub" to chat.peerNpub,
        )
    }

    @Test
    fun send_message_from_args() {
        ensureChatList()
        val peerInput = requiredArg("peer_input")
        val message = requiredArg("message")
        val chat = ensureChatOpen(peerInput)

        composeRule.onNodeWithTag("chatMessageInput", useUnmergedTree = true).performTextInput(message)
        composeRule.onNodeWithTag("chatSendButton", useUnmergedTree = true).performClick()

        waitForState("outgoing message") {
            appManager()
                .state
                .value
                .currentChat
                ?.takeIf { current ->
                    current.chatId == chat.chatId &&
                        current.messages.any { entry ->
                            entry.isOutgoing && entry.body == message
                        }
                }
        }

        val finalized =
            waitForState("message publish", timeoutMs = 180_000) {
                appManager()
                    .state
                    .value
                    .currentChat
                    ?.takeIf { current -> current.chatId == chat.chatId }
                    ?.messages
                    ?.find { entry ->
                        entry.isOutgoing &&
                            entry.body == message &&
                            entry.delivery != DeliveryState.PENDING
                    }
            }

        if (finalized.delivery == DeliveryState.FAILED) {
            fail("Outgoing message failed to publish")
        }

        appManager().state.value.toast?.takeIf { it.isNotBlank() }?.let { toast ->
            fail("Unexpected toast after send: $toast")
        }

        reportStatus(
            "chat_id" to chat.chatId,
            "message" to message,
            "delivery" to finalized.delivery.name,
        )
    }

    @Test
    fun wait_for_message_from_args() {
        ensureChatList()
        val expectedMessage = requiredArg("message")
        val peerInput = arguments.getString("peer_input").orEmpty()

        if (peerInput.isNotBlank()) {
            ensureChatOpen(peerInput)
        }

        waitForState("incoming message", timeoutMs = 180_000) {
            val state = appManager().state.value
            state.currentChat?.takeIf { chat ->
                chat.messages.any { entry ->
                    !entry.isOutgoing && entry.body == expectedMessage
                }
            } ?: state.chatList.firstOrNull { thread ->
                thread.lastMessagePreview == expectedMessage
            }?.also { thread ->
                appManager().openChat(thread.chatId)
            }?.let { null }
        }

        val current =
            waitForState("opened incoming chat", timeoutMs = 30_000) {
                appManager()
                    .state
                    .value
                    .currentChat
                    ?.takeIf { chat ->
                        chat.messages.any { entry ->
                            !entry.isOutgoing && entry.body == expectedMessage
                        }
                    }
            }

        reportStatus(
            "chat_id" to current.chatId,
            "message" to expectedMessage,
        )
    }

    private fun ensureChatList() {
        composeRule.waitUntil(30_000) {
            hasTag("generateKeyButton") ||
                hasTag("chatListNewChatButton") ||
                hasTag("newChatPeerInput") ||
                hasTag("chatMessageInput") ||
                hasTag("myProfileSheet")
        }

        if (hasTag("generateKeyButton")) {
            composeRule.onNodeWithTag("generateKeyButton", useUnmergedTree = true).performClick()
            composeRule.waitForTag("chatListNewChatButton")
            return
        }

        repeat(3) {
            if (hasTag("chatListNewChatButton")) {
                return
            }
            if (hasTag("newChatPeerInput") || hasTag("chatMessageInput") || hasTag("myProfileSheet")) {
                pressBack()
                composeRule.waitForIdle()
            }
        }

        composeRule.waitForTag("chatListNewChatButton")
    }

    private fun ensureChatOpen(peerInput: String): CurrentChatSnapshot {
        val normalized = normalizePeerInput(peerInput)
        val existing = appManager().state.value.chatList.firstOrNull { it.chatId.equals(normalized, true) }
        if (existing != null) {
            composeRule.onNodeWithTag("chatRow-${existing.chatId.take(12)}", useUnmergedTree = true)
                .performClick()
            composeRule.waitForTag("chatMessageInput")
            return waitForState("existing chat") {
                appManager()
                    .state
                    .value
                    .currentChat
                    ?.takeIf { current -> current.chatId.equals(normalized, true) }
            }
        }

        composeRule.onNodeWithTag("chatListNewChatButton", useUnmergedTree = true).performClick()
        composeRule.waitForTag("newChatPeerInput")
        composeRule.onNodeWithTag("newChatPeerInput", useUnmergedTree = true).performTextInput(peerInput)
        composeRule.onNodeWithTag("newChatStartButton", useUnmergedTree = true).performClick()
        composeRule.waitForTag("chatMessageInput")
        return waitForState("created chat") {
            appManager()
                .state
                .value
                .currentChat
                ?.takeIf { current -> current.chatId.equals(normalized, true) }
        }
    }

    private fun requiredArg(name: String): String =
        arguments.getString(name)?.trim()?.takeIf { it.isNotEmpty() }
            ?: throw AssertionError("Missing instrumentation argument: $name")

    private fun reportStatus(vararg fields: Pair<String, String>) {
        val bundle = Bundle()
        fields.forEach { (key, value) -> bundle.putString(key, value) }
        instrumentation.sendStatus(0, bundle)
    }

    private fun <T> waitForState(
        label: String,
        timeoutMs: Long = 60_000,
        condition: () -> T?,
    ): T {
        var result: T? = null
        composeRule.waitUntil(timeoutMs) {
            result = condition()
            result != null
        }
        return result ?: throw AssertionError("Timed out waiting for $label")
    }

    private fun hasTag(tag: String): Boolean =
        composeRule.onAllNodesWithTag(tag, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()

    private fun androidx.compose.ui.test.junit4.AndroidComposeTestRule<*, *>.waitForTag(
        tag: String,
        timeoutMillis: Long = 15_000,
    ) {
        waitUntil(timeoutMillis) {
            onAllNodesWithTag(tag, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()
        }
    }

    @Suppress("unused")
    private fun androidx.compose.ui.test.junit4.AndroidComposeTestRule<*, *>.waitForText(
        text: String,
        timeoutMillis: Long = 15_000,
    ) {
        waitUntil(timeoutMillis) {
            onAllNodesWithText(text, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()
        }
    }
}
