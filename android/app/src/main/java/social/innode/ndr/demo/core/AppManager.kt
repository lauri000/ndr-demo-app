package social.innode.ndr.demo.core

import android.content.Context
import android.util.Base64
import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStoreFile
import java.io.IOException
import java.io.File
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.runBlocking
import social.innode.ndr.demo.BuildConfig
import social.innode.ndr.demo.account.AccountBootstrapState
import social.innode.ndr.demo.account.AccountState
import social.innode.ndr.demo.account.AndroidKeystoreSecretStore
import social.innode.ndr.demo.account.EncryptedSecret
import social.innode.ndr.demo.account.SecureSecretStore
import social.innode.ndr.demo.account.StoredAccountBundle
import social.innode.ndr.demo.rust.AppAction
import social.innode.ndr.demo.rust.AppReconciler
import social.innode.ndr.demo.rust.AppState
import social.innode.ndr.demo.rust.AppUpdate
import social.innode.ndr.demo.rust.FfiApp
import social.innode.ndr.demo.rust.OutgoingAttachment
import social.innode.ndr.demo.rust.Screen

interface RustAppClient {
    fun state(): AppState

    fun dispatch(action: AppAction)

    fun exportSupportBundleJson(): String

    fun listenForUpdates(reconciler: AppReconciler)

    fun shutdown()
}

private class LiveRustAppClient(
    dataDir: String,
    appVersion: String,
) : RustAppClient {
    private val ffi = FfiApp(dataDir = dataDir, keychainGroup = "", appVersion = appVersion)

    override fun state(): AppState = ffi.state()

    override fun dispatch(action: AppAction) {
        ffi.dispatch(action)
    }

    override fun exportSupportBundleJson(): String = ffi.exportSupportBundleJson()

    override fun listenForUpdates(reconciler: AppReconciler) {
        ffi.listenForUpdates(reconciler)
    }

    override fun shutdown() {
        ffi.shutdown()
    }
}

class AppManager(
    context: Context,
    private val applicationScope: CoroutineScope,
    private val secureSecretStore: SecureSecretStore = AndroidKeystoreSecretStore(),
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
    dataStoreName: String = DATASTORE_NAME,
    dataStore: DataStore<Preferences>? = null,
    private val rustFactory: ((dataDir: String, appVersion: String) -> RustAppClient)? = null,
) {
    private val appContext = context.applicationContext
    private val dataStore =
        dataStore
            ?: PreferenceDataStoreFactory.create(
                produceFile = { appContext.preferencesDataStoreFile(dataStoreName) },
            )

    private var rust = createRustApp()
    private var rustGeneration: Long = 0

    private var lastRevApplied: ULong = 0u
    private var restoreCheckComplete = false

    private val mutableState = MutableStateFlow(rust.state())
    val state: StateFlow<AppState> = mutableState.asStateFlow()

    private val mutableBootstrapState =
        MutableStateFlow<AccountBootstrapState>(AccountBootstrapState.Loading)
    val bootstrapState: StateFlow<AccountBootstrapState> = mutableBootstrapState.asStateFlow()

    init {
        val initial = bindRust(rust)
        Log.d(TAG, "init rev=${initial.rev} defaultScreen=${initial.router.defaultScreen}")
        publishState(initial)
        applicationScope.launch(ioDispatcher) {
            restoreSessionFromSecureStore()
        }
    }

    fun createAccount() {
        createAccount("")
    }

    fun createAccount(name: String) {
        rust.dispatch(AppAction.CreateAccount(name.trim()))
    }

    fun restoreSession(nsecOrHex: String) {
        val trimmed = nsecOrHex.trim()
        if (trimmed.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.RestoreSession(trimmed))
    }

    fun startLinkedDevice(ownerInput: String) {
        val trimmed = ownerInput.trim()
        if (trimmed.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.StartLinkedDevice(trimmed))
    }

    fun addAuthorizedDevice(deviceInput: String) {
        val trimmed = deviceInput.trim()
        if (trimmed.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.AddAuthorizedDevice(trimmed))
    }

    fun removeAuthorizedDevice(devicePubkeyHex: String) {
        val trimmed = devicePubkeyHex.trim()
        if (trimmed.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.RemoveAuthorizedDevice(trimmed))
    }

    fun dispatch(action: AppAction) {
        rust.dispatch(action)
    }

    fun appForegrounded() {
        rust.dispatch(AppAction.AppForegrounded)
    }

    fun createChat(peerInput: String) {
        val trimmed = peerInput.trim()
        if (trimmed.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.CreateChat(trimmed))
    }

    fun createGroup(
        name: String,
        memberInputs: List<String>,
    ) {
        val trimmedName = name.trim()
        val trimmedMembers = memberInputs.map(String::trim).filter(String::isNotEmpty)
        if (trimmedName.isEmpty() || trimmedMembers.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.CreateGroup(trimmedName, trimmedMembers))
    }

    fun updateGroupName(
        groupId: String,
        name: String,
    ) {
        val trimmedGroupId = groupId.trim()
        val trimmedName = name.trim()
        if (trimmedGroupId.isEmpty() || trimmedName.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.UpdateGroupName(trimmedGroupId, trimmedName))
    }

    fun addGroupMembers(
        groupId: String,
        memberInputs: List<String>,
    ) {
        val trimmedGroupId = groupId.trim()
        val trimmedMembers = memberInputs.map(String::trim).filter(String::isNotEmpty)
        if (trimmedGroupId.isEmpty() || trimmedMembers.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.AddGroupMembers(trimmedGroupId, trimmedMembers))
    }

    fun removeGroupMember(
        groupId: String,
        ownerPubkeyHex: String,
    ) {
        val trimmedGroupId = groupId.trim()
        val trimmedOwner = ownerPubkeyHex.trim()
        if (trimmedGroupId.isEmpty() || trimmedOwner.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.RemoveGroupMember(trimmedGroupId, trimmedOwner))
    }

    fun openChat(chatId: String) {
        val trimmed = chatId.trim()
        if (trimmed.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.OpenChat(trimmed))
    }

    fun pushScreen(screen: Screen) {
        rust.dispatch(AppAction.PushScreen(screen))
    }

    fun sendText(
        chatId: String,
        text: String,
    ) {
        val trimmedChatId = chatId.trim()
        val trimmedText = text.trim()
        if (trimmedChatId.isEmpty() || trimmedText.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.SendMessage(trimmedChatId, trimmedText))
    }

    fun sendAttachment(
        chatId: String,
        filePath: String,
        filename: String,
        caption: String,
    ) {
        val trimmedChatId = chatId.trim()
        val trimmedPath = filePath.trim()
        val trimmedFilename = filename.trim()
        if (trimmedChatId.isEmpty() || trimmedPath.isEmpty() || trimmedFilename.isEmpty()) {
            return
        }
        rust.dispatch(
            AppAction.SendAttachment(
                trimmedChatId,
                trimmedPath,
                trimmedFilename,
                caption.trim(),
            ),
        )
    }

    fun sendAttachments(
        chatId: String,
        attachments: List<OutgoingAttachment>,
        caption: String,
    ) {
        val trimmedChatId = chatId.trim()
        val outgoing =
            attachments
                .map {
                    OutgoingAttachment(
                        filePath = it.filePath.trim(),
                        filename = it.filename.trim(),
                    )
                }.filter { it.filePath.isNotEmpty() && it.filename.isNotEmpty() }
        if (trimmedChatId.isEmpty() || outgoing.isEmpty()) {
            return
        }
        rust.dispatch(
            AppAction.SendAttachments(
                trimmedChatId,
                outgoing,
                caption.trim(),
            ),
        )
    }

    fun logout() {
        applicationScope.launch(ioDispatcher) {
            // Logout is owned by Rust. The shell clears native secrets and then swaps in a fresh core
            // instead of fabricating a shell-authored logged-out snapshot.
            rust.dispatch(AppAction.Logout)
            clearPersistedSecret()
            secureSecretStore.clear()
            replaceRustCoreAfterReset()
        }
    }

    suspend fun exportNsec(): String? =
        withContext(ioDispatcher) {
            loadPersistedBundle()?.ownerNsec
        }

    suspend fun exportSupportBundleJson(): String =
        withContext(ioDispatcher) {
            rust.exportSupportBundleJson()
        }

    fun resetAppState() {
        logout()
    }

    fun resetForUiTestsBlocking() {
        runBlocking(ioDispatcher) {
            clearPersistedSecret()
            secureSecretStore.clear()
            replaceRustCoreAfterReset()
        }
    }

    fun buildSummary(): String = "${BuildConfig.VERSION_NAME} (${BuildConfig.BUILD_GIT_SHA})"

    fun relaySetId(): String = BuildConfig.RELAY_SET_ID

    fun isTrustedTestBuild(): Boolean = BuildConfig.TRUSTED_TEST_BUILD

    private fun applyUpdate(update: AppUpdate) {
        when (update) {
            is AppUpdate.PersistAccountBundle -> {
                // Secure persistence is a shell side effect and must be applied even if snapshot revs race.
                applicationScope.launch(ioDispatcher) {
                    persistBundle(
                        StoredAccountBundle(
                            ownerNsec = update.ownerNsec,
                            ownerPubkeyHex = update.ownerPubkeyHex,
                            deviceNsec = update.deviceNsec,
                        ),
                    )
                }
            }
            is AppUpdate.FullState -> {
                // Rust owns authoritative state. The shell only accepts the newest full snapshot.
                if (update.v1.rev <= lastRevApplied) {
                    return
                }
                lastRevApplied = update.v1.rev
                Log.d(
                    TAG,
                    "reconcile rev=${update.v1.rev} screen=${update.v1.router.defaultScreen} " +
                        "chatList=${update.v1.chatList.size} activeChat=${update.v1.currentChat?.chatId.orEmpty()} " +
                        "toast=${update.v1.toast.orEmpty()}",
                )
                publishState(update.v1)
            }
        }
    }

    private suspend fun restoreSessionFromSecureStore() {
        // Native restore only rehydrates secure inputs. Rust rebuilds the authoritative app state.
        Log.d(TAG, "restoreSessionFromSecureStore start")
        val encrypted = loadPersistedSecret()
        if (encrypted == null) {
            Log.d(TAG, "restoreSessionFromSecureStore no persisted secret")
            restoreCheckComplete = true
            publishBootstrapNeedsLogin()
            return
        }

        val decrypted = runCatching { secureSecretStore.decrypt(encrypted).decodeToString() }.getOrNull()
        if (decrypted.isNullOrBlank()) {
            Log.d(TAG, "restoreSessionFromSecureStore decrypt failed or blank")
            clearPersistedSecret()
            restoreCheckComplete = true
            publishBootstrapNeedsLogin()
            return
        }

        restoreCheckComplete = true
        val bundle = StoredAccountBundle.fromJson(decrypted)
        if (bundle != null) {
            Log.d(TAG, "restoreSessionFromSecureStore dispatch bundle restore")
            rust.dispatch(
                AppAction.RestoreAccountBundle(
                    ownerNsec = bundle.ownerNsec,
                    ownerPubkeyHex = bundle.ownerPubkeyHex,
                    deviceNsec = bundle.deviceNsec,
                ),
            )
        } else {
            Log.d(TAG, "restoreSessionFromSecureStore dispatch direct restore")
            rust.dispatch(AppAction.RestoreSession(decrypted))
        }
    }

    private fun bindRust(client: RustAppClient): AppState {
        rust = client
        rustGeneration += 1
        val generation = rustGeneration
        val initial = client.state()
        lastRevApplied = initial.rev
        client.listenForUpdates(UpdateBridge(generation))
        return initial
    }

    private suspend fun persistBundle(bundle: StoredAccountBundle) {
        val encrypted = secureSecretStore.encrypt(bundle.toJson().encodeToByteArray())
        dataStore.edit { preferences ->
            preferences[SECRET_CIPHERTEXT] = encrypted.cipherText.toBase64()
            preferences[SECRET_IV] = encrypted.iv.toBase64()
        }
    }

    private suspend fun loadPersistedBundle(): StoredAccountBundle? {
        val encrypted = loadPersistedSecret() ?: return null
        val decrypted = runCatching { secureSecretStore.decrypt(encrypted).decodeToString() }.getOrNull()
            ?: return null
        return StoredAccountBundle.fromJson(decrypted)
    }

    private suspend fun loadPersistedSecret(): EncryptedSecret? {
        val preferences =
            dataStore.data
                .catch { throwable ->
                    if (throwable is IOException) {
                        emit(emptyPreferences())
                    } else {
                        throw throwable
                    }
                }.first()

        val cipherText = preferences[SECRET_CIPHERTEXT] ?: return null
        val iv = preferences[SECRET_IV] ?: return null
        return EncryptedSecret(
            cipherText = cipherText.fromBase64(),
            iv = iv.fromBase64(),
        )
    }

    private suspend fun clearPersistedSecret() {
        dataStore.edit { preferences ->
            preferences.remove(SECRET_CIPHERTEXT)
            preferences.remove(SECRET_IV)
        }
    }

    private fun replaceRustCoreAfterReset() {
        val previous = rust
        previous.shutdown()
        wipeAppStorage()
        val initial = bindRust(createRustApp())
        restoreCheckComplete = true
        publishState(initial)
    }

    private fun wipeAppStorage() {
        wipeDirectoryContents(appContext.filesDir)
        wipeDirectoryContents(appContext.noBackupFilesDir)
        appContext.getExternalFilesDirs(null).forEach { dir ->
            if (dir != null) {
                wipeDirectoryContents(dir)
            }
        }
        appContext.filesDir.mkdirs()
        appContext.noBackupFilesDir.mkdirs()
    }

    private fun wipeDirectoryContents(directory: File?) {
        val dir = directory ?: return
        if (!dir.exists()) {
            return
        }
        dir.listFiles()?.forEach { child ->
            runCatching { child.deleteRecursively() }
        }
    }

    private fun publishState(snapshot: AppState) {
        mutableState.value = snapshot
        if (!restoreCheckComplete) {
            mutableBootstrapState.value = AccountBootstrapState.Loading
            return
        }
        val account = snapshot.account
        mutableBootstrapState.value =
            when {
                account != null ->
                    AccountBootstrapState.LoggedIn(
                        AccountState(
                            publicKeyHex = account.publicKeyHex,
                            npub = account.npub,
                        ),
                    )
                snapshot.busy.restoringSession -> AccountBootstrapState.Loading
                else -> AccountBootstrapState.NeedsLogin
            }
    }

    private fun publishBootstrapNeedsLogin() {
        restoreCheckComplete = true
        Log.d(TAG, "bootstrap needs login")
        mutableBootstrapState.value = AccountBootstrapState.NeedsLogin
    }

    private fun ByteArray.toBase64(): String = Base64.encodeToString(this, Base64.NO_WRAP)

    private fun String.fromBase64(): ByteArray = Base64.decode(this, Base64.NO_WRAP)

    private companion object {
        const val TAG = "NdrDebug"
        const val DATASTORE_NAME = "ndr_demo_secure_store.preferences_pb"
        val SECRET_CIPHERTEXT = stringPreferencesKey("secret_ciphertext")
        val SECRET_IV = stringPreferencesKey("secret_iv")

        fun appVersion(context: Context): String =
            runCatching {
                context.packageManager.getPackageInfo(context.packageName, 0).versionName
            }.getOrNull()
                ?: "0.1.0"
    }

    private fun createRustApp(): RustAppClient =
        rustFactory?.invoke(appContext.filesDir.absolutePath, appVersion(appContext))
            ?: LiveRustAppClient(
                dataDir = appContext.filesDir.absolutePath,
                appVersion = appVersion(appContext),
            )

    private inner class UpdateBridge(
        private val generation: Long,
    ) : AppReconciler {
        override fun reconcile(update: AppUpdate) {
            if (generation != rustGeneration) {
                return
            }
            applyUpdate(update)
        }
    }
}
