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
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextInput
import androidx.test.espresso.Espresso.pressBack
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import social.innode.ndr.demo.qr.DeviceApprovalQr
import social.innode.ndr.demo.ui.screens.QrScannerTestOverrides

@RunWith(AndroidJUnit4::class)
class PikaLikeUiTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Before
    fun resetAppState() {
        QrScannerTestOverrides.nextScannedValue = null
        (composeRule.activity.application as IrisChatApp)
            .container
            .appManager
            .resetForUiTestsBlocking()
        composeRule.waitUntil(20_000) {
            composeRule
                .onAllNodesWithTag("generateKeyButton", useUnmergedTree = true)
                .fetchSemanticsNodes()
                .isNotEmpty()
        }
    }

    @Test
    fun generate_account_and_open_profile_sheet() {
        composeRule.ensureChatList()
        composeRule.onNodeWithTag("chatListProfileButton", useUnmergedTree = true).performClick()

        composeRule.waitForTag("myProfileSheet")
        composeRule.onNodeWithTag("myProfileQrCode", useUnmergedTree = true).assertIsDisplayed()
        composeRule.onNodeWithTag("myProfileManageDevicesButton", useUnmergedTree = true).assertIsDisplayed()
    }

    @Test
    fun profile_sheet_opens_manage_devices() {
        composeRule.ensureChatList()
        composeRule.onNodeWithTag("chatListProfileButton", useUnmergedTree = true).performClick()

        composeRule.waitForTag("myProfileManageDevicesButton")
        composeRule.onNodeWithTag("myProfileManageDevicesButton", useUnmergedTree = true)
            .performClick()

        composeRule.waitForTag("deviceRosterOwnerNpub")
        composeRule.onNodeWithTag("deviceRosterCurrentDeviceNpub", useUnmergedTree = true)
            .assertIsDisplayed()
        composeRule.onNodeWithTag("deviceRosterAddInput", useUnmergedTree = true).assertIsDisplayed()
    }

    @Test
    fun manage_devices_valid_input_enables_authorize_action() {
        composeRule.ensureChatList()
        composeRule.onNodeWithTag("chatListProfileButton", useUnmergedTree = true).performClick()
        composeRule.waitForTag("myProfileManageDevicesButton")
        composeRule.onNodeWithTag("myProfileManageDevicesButton", useUnmergedTree = true)
            .performClick()

        composeRule.waitForTag("deviceRosterAddInput")
        composeRule.onNodeWithTag("deviceRosterAddInput", useUnmergedTree = true)
            .performTextInput(SECONDARY_DEVICE_NPUB)
        composeRule.onNodeWithTag("deviceRosterAddButton", useUnmergedTree = true)
            .assertIsEnabled()
    }

    @Test
    fun scan_device_approval_qr_authorizes_device() {
        composeRule.ensureChatList()
        composeRule.onNodeWithTag("chatListProfileButton", useUnmergedTree = true).performClick()
        composeRule.waitForTag("myProfileManageDevicesButton")
        composeRule.onNodeWithTag("myProfileManageDevicesButton", useUnmergedTree = true)
            .performClick()

        composeRule.waitForTag("deviceRosterOwnerNpub")
        val ownerNpub =
            (composeRule.activity.application as IrisChatApp)
                .container
                .appManager
                .state
                .value
                .deviceRoster
                ?.ownerNpub
                .orEmpty()

        composeRule.runOnUiThread {
            QrScannerTestOverrides.nextScannedValue =
                DeviceApprovalQr.encode(
                    ownerInput = ownerNpub,
                    deviceInput = SECONDARY_DEVICE_NPUB,
                )
        }
        composeRule.onNodeWithTag("deviceRosterScanButton", useUnmergedTree = true).performClick()
        composeRule.waitUntil(10_000) {
            runCatching {
                composeRule
                    .onNodeWithTag("deviceRosterAddButton", useUnmergedTree = true)
                    .assertIsEnabled()
                true
            }.getOrDefault(false)
        }
        composeRule.onNodeWithTag("deviceRosterAddButton", useUnmergedTree = true).performClick()
        composeRule.waitUntil(20_000) {
            val roster =
                (composeRule.activity.application as IrisChatApp)
                    .container
                    .appManager
                    .state
                    .value
                    .deviceRoster
            roster?.devices?.any { it.deviceNpub == SECONDARY_DEVICE_NPUB && it.isAuthorized } == true
        }
    }

    @Test
    fun create_chat_and_send_message_locally() {
        composeRule.ensureChatList()
        composeRule.onNodeWithTag("chatListNewChatButton", useUnmergedTree = true).performClick()
        composeRule.waitForTag("chatListNewChatOption")
        composeRule.onNodeWithTag("chatListNewChatOption", useUnmergedTree = true).performClick()

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
        composeRule.waitForTag("chatListNewChatOption")
        composeRule.onNodeWithTag("chatListNewChatOption", useUnmergedTree = true).performClick()

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

    @Test
    fun scan_owner_qr_enters_awaiting_approval_screen() {
        composeRule.resetToWelcome()
        composeRule.waitForTag("linkOwnerInput")
        composeRule.runOnUiThread {
            QrScannerTestOverrides.nextScannedValue = VALID_PEER_NPUB
        }
        composeRule.onNodeWithTag("linkOwnerScanQrButton", useUnmergedTree = true).performClick()
        composeRule.onNodeWithTag("linkExistingAccountButton", useUnmergedTree = true).performClick()

        composeRule.waitForTag("awaitingApprovalScreen")
        composeRule.onNodeWithTag("awaitingApprovalDeviceQrCode", useUnmergedTree = true)
            .assertIsDisplayed()
        composeRule.onNodeWithTag("awaitingApprovalDeviceNpub", useUnmergedTree = true)
            .assertIsDisplayed()
    }

    @Test
    fun chat_list_new_chooser_opens_group_flow() {
        composeRule.ensureChatList()
        composeRule.onNodeWithTag("chatListNewChatButton", useUnmergedTree = true).performClick()

        composeRule.waitForTag("chatListNewGroupOption")
        composeRule.onNodeWithTag("chatListNewGroupOption", useUnmergedTree = true).performClick()

        composeRule.waitForTag("newGroupNameInput")
        composeRule.onNodeWithTag("newGroupCreateButton", useUnmergedTree = true).assertIsNotEnabled()
    }

    @Test
    fun create_group_and_open_group_details() {
        composeRule.ensureChatList()
        composeRule.onNodeWithTag("chatListNewChatButton", useUnmergedTree = true).performClick()
        composeRule.waitForTag("chatListNewGroupOption")
        composeRule.onNodeWithTag("chatListNewGroupOption", useUnmergedTree = true).performClick()

        composeRule.waitForTag("newGroupNameInput")
        composeRule.onNodeWithTag("newGroupNameInput", useUnmergedTree = true)
            .performTextInput("Trip crew")
        composeRule.onNodeWithTag("newGroupMemberInput", useUnmergedTree = true)
            .performTextInput(VALID_PEER_NPUB)
        composeRule.onNodeWithTag("newGroupAddMemberButton", useUnmergedTree = true).performClick()
        composeRule.onNodeWithTag("newGroupCreateButton", useUnmergedTree = true).assertIsEnabled()
        composeRule.onNodeWithTag("newGroupCreateButton", useUnmergedTree = true).performClick()

        composeRule.waitForTag("chatMessageInput")
        composeRule.onNodeWithTag("chatGroupDetailsButton", useUnmergedTree = true).performClick()

        composeRule.waitForTag("groupDetailsScreen")
        composeRule.onNodeWithTag("groupDetailsNameInput", useUnmergedTree = true).assertIsDisplayed()
        composeRule.onNodeWithTag("groupDetailsAddMembersButton", useUnmergedTree = true)
            .performScrollTo()
            .assertIsDisplayed()
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
        private const val SECONDARY_DEVICE_NPUB =
            "npub1p34efzmkewwdsksmpp2r0tk7quke9jcfdz2zl7ezk8wnsj43uz2s8x5sp4"
    }

    private fun androidx.compose.ui.test.junit4.AndroidComposeTestRule<*, *>.ensureChatList() {
        waitUntil(30_000) {
            hasTag("generateKeyButton") ||
                hasTag("chatListNewChatButton") ||
                hasTag("newChatPeerInput") ||
                hasTag("newGroupNameInput") ||
                hasTag("chatMessageInput") ||
                hasTag("myProfileSheet") ||
                hasTag("groupDetailsScreen")
        }

        if (hasTag("generateKeyButton")) {
            onNodeWithTag("signupNameField", useUnmergedTree = true)
                .performTextInput("android tester")
            onNodeWithTag("generateKeyButton", useUnmergedTree = true).performClick()
            waitForTag("chatListNewChatButton")
            return
        }

        repeat(3) {
            if (hasTag("chatListNewChatButton")) {
                return
            }
            if (
                hasTag("newChatPeerInput") ||
                hasTag("newGroupNameInput") ||
                hasTag("chatMessageInput") ||
                hasTag("myProfileSheet") ||
                hasTag("groupDetailsScreen")
            ) {
                pressBack()
                waitForIdle()
            }
        }

        waitForTag("chatListNewChatButton")
    }

    private fun androidx.compose.ui.test.junit4.AndroidComposeTestRule<*, *>.resetToWelcome() {
        runOnUiThread {
            val activity = activity
            (activity.application as IrisChatApp).container.appManager.logout()
        }
        waitForTag("generateKeyButton", timeoutMillis = 30_000)
    }

    private fun androidx.compose.ui.test.junit4.AndroidComposeTestRule<*, *>.hasTag(tag: String): Boolean =
        onAllNodesWithTag(tag, useUnmergedTree = true).fetchSemanticsNodes().isNotEmpty()
}
