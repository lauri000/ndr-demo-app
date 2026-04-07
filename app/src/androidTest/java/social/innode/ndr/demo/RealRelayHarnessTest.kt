package social.innode.ndr.demo

import android.os.Bundle
import android.os.SystemClock
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.rules.ActivityScenarioRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import social.innode.ndr.demo.core.AppManager
import social.innode.ndr.demo.account.AccountBootstrapState
import social.innode.ndr.demo.rust.CurrentChatSnapshot
import social.innode.ndr.demo.rust.DeliveryState
import social.innode.ndr.demo.rust.normalizePeerInput

@RunWith(AndroidJUnit4::class)
class RealRelayHarnessTest {
    @get:Rule
    val activityRule = ActivityScenarioRule(MainActivity::class.java)

    private val instrumentation
        get() = InstrumentationRegistry.getInstrumentation()

    private val arguments
        get() = InstrumentationRegistry.getArguments()

    private fun appManager(): AppManager =
        withActivity { (it.application as NdrDemoApp).container.appManager }

    private fun <T> withActivity(block: (MainActivity) -> T): T {
        var result: Result<T>? = null
        activityRule.scenario.onActivity { activity ->
            result = runCatching { block(activity) }
        }
        return result?.getOrThrow() ?: error("Activity was not available")
    }

    @Test
    fun create_account_and_report_identity() {
        val account = ensureLoggedIn()
        reportStatus(
            "npub" to account.npub,
            "public_key_hex" to account.publicKeyHex,
            "app_package" to withActivity { it.packageName },
            "data_dir" to withActivity { it.filesDir.absolutePath },
        )
    }

    @Test
    fun create_chat_from_args() {
        ensureLoggedIn()
        val peerInput = requiredArg("peer_input")
        val chat = ensureChatOpen(peerInput)
        reportStatus(
            "chat_id" to chat.chatId,
            "peer_npub" to chat.peerNpub,
        )
    }

    @Test
    fun send_message_from_args() {
        ensureLoggedIn()
        val peerInput = requiredArg("peer_input")
        val message = requiredArg("message")
        val chat = ensureChatOpen(peerInput)

        appManager().sendText(chat.chatId, message)

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
        ensureLoggedIn()
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

    @Test
    fun logout_and_create_account_and_report_identity() {
        val oldAccount = ensureLoggedIn()
        appManager().logout()

        waitForState("logged out state", timeoutMs = 60_000) {
            appManager().state.value.takeIf { it.account == null }
        }

        val filesEntries = storageEntries(withActivity { it.filesDir })
        if (filesEntries.isNotEmpty()) {
            fail("Expected filesDir to be empty after logout, found: $filesEntries")
        }

        appManager().createAccount()

        val newAccount = waitForState("new account") { appManager().state.value.account }
        if (newAccount.publicKeyHex.equals(oldAccount.publicKeyHex, ignoreCase = true)) {
            fail("Expected a fresh identity after logout")
        }

        reportStatus(
            "old_public_key_hex" to oldAccount.publicKeyHex,
            "new_public_key_hex" to newAccount.publicKeyHex,
            "new_npub" to newAccount.npub,
        )
    }

    private fun ensureLoggedIn(): social.innode.ndr.demo.rust.AccountSnapshot {
        var createRequested = false
        return waitForState("logged in account", timeoutMs = 90_000) {
            val manager = appManager()
            manager.state.value.account?.let { return@waitForState it }

            when (manager.bootstrapState.value) {
                AccountBootstrapState.Loading -> null
                AccountBootstrapState.NeedsLogin -> {
                    if (!createRequested) {
                        createRequested = true
                        manager.createAccount()
                    }
                    null
                }
                is AccountBootstrapState.LoggedIn -> null
            }
        }
    }

    private fun ensureChatOpen(peerInput: String): CurrentChatSnapshot {
        val existing =
            appManager().state.value.chatList.firstOrNull { thread ->
                matchesPeerInput(
                    chatId = thread.chatId,
                    peerNpub = thread.peerNpub,
                    peerInput = peerInput,
                )
            }
        if (existing != null) {
            appManager().openChat(existing.chatId)
            return waitForState("existing chat") {
                appManager()
                    .state
                    .value
                    .currentChat
                    ?.takeIf { current -> matchesPeerInput(current.chatId, current.peerNpub, peerInput) }
            }
        }

        appManager().createChat(peerInput)
        return waitForState("created chat") {
            appManager()
                .state
                .value
                .currentChat
                ?.takeIf { current -> matchesPeerInput(current.chatId, current.peerNpub, peerInput) }
        }
    }

    private fun matchesPeerInput(
        chatId: String,
        peerNpub: String,
        peerInput: String,
    ): Boolean {
        val normalized = normalizePeerInput(peerInput)
        return chatId.equals(normalized, ignoreCase = true) ||
            peerNpub.equals(normalized, ignoreCase = true)
    }

    private fun requiredArg(name: String): String =
        arguments.getString(name)?.trim()?.takeIf { it.isNotEmpty() }
            ?: throw AssertionError("Missing instrumentation argument: $name")

    private fun storageEntries(root: File): List<String> =
        root
            .listFiles()
            ?.sortedBy { it.name }
            ?.map { it.relativeTo(root).path.ifBlank { it.name } }
            ?: emptyList()

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
        val deadline = SystemClock.elapsedRealtime() + timeoutMs
        while (SystemClock.elapsedRealtime() < deadline) {
            condition()?.let { return it }
            instrumentation.waitForIdleSync()
            SystemClock.sleep(100)
        }
        throw AssertionError("Timed out waiting for $label")
    }
}
