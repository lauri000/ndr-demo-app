package social.innode.ndr.demo.core

import android.content.Context
import android.util.Base64
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
import social.innode.ndr.demo.account.AccountBootstrapState
import social.innode.ndr.demo.account.AccountState
import social.innode.ndr.demo.account.AndroidKeystoreSecretStore
import social.innode.ndr.demo.account.EncryptedSecret
import social.innode.ndr.demo.account.SecureSecretStore
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
        rust.listenForUpdates(this)
        applicationScope.launch(ioDispatcher) {
            restoreSessionFromSecureStore()
        }
    }

    fun createAccount() {
        rust.dispatch(AppAction.CreateAccount)
    }

    fun restoreSession(nsecOrHex: String) {
        val trimmed = nsecOrHex.trim()
        if (trimmed.isEmpty()) {
            return
        }
        rust.dispatch(AppAction.RestoreSession(trimmed))
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
            val encrypted = loadPersistedSecret() ?: return@withContext null
            secureSecretStore.decrypt(encrypted).decodeToString()
        }

    override fun reconcile(update: AppUpdate) {
        when (update) {
            is AppUpdate.AccountCreated -> {
                applicationScope.launch(ioDispatcher) {
                    persistSecret(update.nsec)
                }
            }
            is AppUpdate.FullState -> {
                if (update.v1.rev <= lastRevApplied) {
                    return
                }
                lastRevApplied = update.v1.rev
                publishState(update.v1)
            }
        }
    }

    private suspend fun restoreSessionFromSecureStore() {
        val encrypted = loadPersistedSecret()
        if (encrypted == null) {
            restoreCheckComplete = true
            publishBootstrapNeedsLogin()
            return
        }

        val nsec = runCatching { secureSecretStore.decrypt(encrypted).decodeToString() }.getOrNull()
        if (nsec.isNullOrBlank()) {
            clearPersistedSecret()
            restoreCheckComplete = true
            publishBootstrapNeedsLogin()
            return
        }

        restoreCheckComplete = true
        rust.dispatch(AppAction.RestoreSession(nsec))
    }

    private suspend fun persistSecret(nsec: String) {
        val encrypted = secureSecretStore.encrypt(nsec.encodeToByteArray())
        dataStore.edit { preferences ->
            preferences[SECRET_CIPHERTEXT] = encrypted.cipherText.toBase64()
            preferences[SECRET_IV] = encrypted.iv.toBase64()
        }
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
                router =
                    Router(
                        defaultScreen = Screen.ChatList,
                        screenStack = emptyList(),
                    )
                busy =
                    BusyState(
                        creatingAccount = false,
                        restoringSession = false,
                        creatingChat = false,
                        sendingMessage = false,
                        syncingNetwork = false,
                    )
                chatList = emptyList()
                currentChat = null
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
        mutableBootstrapState.value = AccountBootstrapState.NeedsLogin
    }

    private fun ByteArray.toBase64(): String = Base64.encodeToString(this, Base64.NO_WRAP)

    private fun String.fromBase64(): ByteArray = Base64.decode(this, Base64.NO_WRAP)

    private companion object {
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
