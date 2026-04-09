package social.innode.ndr.demo.core

import android.content.Context
import android.util.Base64
import android.util.Log
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
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
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
import social.innode.ndr.demo.rust.BusyState
import social.innode.ndr.demo.rust.FfiApp
import social.innode.ndr.demo.rust.Router
import social.innode.ndr.demo.rust.Screen

class AppManager(
    context: Context,
    private val applicationScope: CoroutineScope,
    private val secureSecretStore: SecureSecretStore = AndroidKeystoreSecretStore(),
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
) : AppReconciler {
    private val appContext = context.applicationContext
    private val dataStore =
        PreferenceDataStoreFactory.create(
            produceFile = { appContext.preferencesDataStoreFile(DATASTORE_NAME) },
        )

    private val rust =
        FfiApp(
            dataDir = appContext.filesDir.absolutePath,
            keychainGroup = "",
            appVersion = appVersion(appContext),
        )

    private var lastRevApplied: ULong = 0u
    private var restoreCheckComplete = false

    private val mutableState = MutableStateFlow(rust.state())
    val state: StateFlow<AppState> = mutableState.asStateFlow()

    private val mutableBootstrapState =
        MutableStateFlow<AccountBootstrapState>(AccountBootstrapState.Loading)
    val bootstrapState: StateFlow<AccountBootstrapState> = mutableBootstrapState.asStateFlow()

    init {
        val initial = rust.state()
        lastRevApplied = initial.rev
        mutableState.value = initial
        Log.d(TAG, "init rev=${initial.rev} defaultScreen=${initial.router.defaultScreen}")
        rust.listenForUpdates(this)
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

    fun logout() {
        applicationScope.launch(ioDispatcher) {
            rust.dispatch(AppAction.Logout)
            clearPersistedSecret()
            secureSecretStore.clear()
            wipeAppStorage()
            publishState(waitForLoggedOutSnapshot())
            restoreCheckComplete = true
            publishBootstrapNeedsLogin()
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

    fun buildSummary(): String = "${BuildConfig.VERSION_NAME} (${BuildConfig.BUILD_GIT_SHA})"

    fun relaySetId(): String = BuildConfig.RELAY_SET_ID

    fun isTrustedTestBuild(): Boolean = BuildConfig.TRUSTED_TEST_BUILD

    override fun reconcile(update: AppUpdate) {
        when (update) {
            is AppUpdate.PersistAccountBundle -> {
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

    private suspend fun waitForLoggedOutSnapshot(): AppState {
        repeat(40) {
            val snapshot = rust.state()
            if (snapshot.account == null) {
                return snapshot
            }
            delay(100)
        }

        return rust
            .state()
            .apply {
                account = null
                deviceRoster = null
                router =
                    Router(
                        defaultScreen = Screen.Welcome,
                        screenStack = emptyList(),
                    )
                busy =
                    BusyState(
                        creatingAccount = false,
                        restoringSession = false,
                        linkingDevice = false,
                        creatingChat = false,
                        creatingGroup = false,
                        sendingMessage = false,
                        updatingRoster = false,
                        updatingGroup = false,
                        syncingNetwork = false,
                    )
                chatList = emptyList()
                currentChat = null
                groupDetails = null
                toast = null
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
}
