import XCTest

final class NdrDemoUITests: XCTestCase {
    private let validPeerNpub = "npub18w35g6gn47qwmryulxzvfucmujvrqqljjpapyl8x0rqaljh6f2usml77dj"

    func testCreateAccountAndOpenProfileSheet() {
        let app = launchCleanApp()

        createAccount(app)

        XCTAssertTrue(element(app, "chatListProfileButton").waitForExistence(timeout: 15))
        element(app, "chatListProfileButton").tap()

        XCTAssertTrue(element(app, "myProfileSheet").waitForExistence(timeout: 10))
        XCTAssertTrue(element(app, "myProfileQrCode").waitForExistence(timeout: 5))
    }

    func testCreateChatAndSendMessageLocally() {
        let app = launchCleanApp()

        createAccount(app)

        app.buttons["chatListNewChatButton"].tap()
        XCTAssertTrue(app.textFields["newChatPeerInput"].waitForExistence(timeout: 10))
        app.textFields["newChatPeerInput"].tap()
        app.textFields["newChatPeerInput"].typeText(validPeerNpub)
        app.buttons["newChatStartButton"].tap()

        XCTAssertTrue(app.textFields["chatMessageInput"].waitForExistence(timeout: 10))
        app.textFields["chatMessageInput"].tap()
        app.textFields["chatMessageInput"].typeText("hello from ios ui test")
        app.buttons["chatSendButton"].tap()

        XCTAssertTrue(app.staticTexts["hello from ios ui test"].waitForExistence(timeout: 15))
    }

    func testCreateGroupAndOpenGroupDetails() {
        let app = launchCleanApp()

        createAccount(app)

        app.buttons["chatListNewGroupButton"].tap()
        XCTAssertTrue(app.textFields["newGroupNameInput"].waitForExistence(timeout: 10))
        app.textFields["newGroupNameInput"].tap()
        app.textFields["newGroupNameInput"].typeText("Trip crew")
        app.textFields["newGroupMemberInput"].tap()
        app.textFields["newGroupMemberInput"].typeText(validPeerNpub)
        app.buttons["newGroupAddMemberButton"].tap()
        app.buttons["newGroupCreateButton"].tap()

        XCTAssertTrue(app.textFields["chatMessageInput"].waitForExistence(timeout: 15))
        app.buttons["chatGroupDetailsButton"].tap()

        XCTAssertTrue(element(app, "groupDetailsScreen").waitForExistence(timeout: 10))
        XCTAssertTrue(element(app, "groupDetailsNameInput").waitForExistence(timeout: 5))
        XCTAssertTrue(element(app, "groupDetailsAddMembersButton").waitForExistence(timeout: 5))
    }

    func testScanOwnerQrEntersAwaitingApprovalScreen() {
        let app = launchCleanApp(qrValue: validPeerNpub)

        XCTAssertTrue(app.buttons["linkOwnerScanQrButton"].waitForExistence(timeout: 10))
        app.buttons["linkOwnerScanQrButton"].tap()
        XCTAssertTrue(app.buttons["linkExistingAccountButton"].waitForExistence(timeout: 10))
        app.buttons["linkExistingAccountButton"].tap()

        XCTAssertTrue(element(app, "awaitingApprovalScreen").waitForExistence(timeout: 20))
        XCTAssertTrue(element(app, "awaitingApprovalDeviceQrCode").waitForExistence(timeout: 10))
        XCTAssertTrue(element(app, "awaitingApprovalDeviceNpub").waitForExistence(timeout: 10))
    }

    private func launchCleanApp(qrValue: String? = nil) -> XCUIApplication {
        let app = XCUIApplication()
        app.launchEnvironment["NDR_UI_TEST_RESET"] = "1"
        app.launchEnvironment["NDR_UI_TEST_RUN_ID"] = UUID().uuidString
        if let qrValue {
            app.launchEnvironment["NDR_QR_TEST_VALUE"] = qrValue
        }
        app.launch()
        XCTAssertTrue(app.wait(for: .runningForeground, timeout: 15))
        return app
    }

    private func createAccount(_ app: XCUIApplication) {
        let nameField = app.textFields["signupNameField"]
        XCTAssertTrue(nameField.waitForExistence(timeout: 15))
        nameField.tap()
        nameField.typeText("ios tester")
        app.buttons["generateKeyButton"].tap()
        XCTAssertTrue(app.buttons["chatListNewChatButton"].waitForExistence(timeout: 20))
    }

    private func element(_ app: XCUIApplication, _ identifier: String) -> XCUIElement {
        app.descendants(matching: .any)[identifier]
    }
}
