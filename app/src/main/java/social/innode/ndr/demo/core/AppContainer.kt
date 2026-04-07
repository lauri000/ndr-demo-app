package social.innode.ndr.demo.core

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import social.innode.ndr.demo.account.AndroidKeystoreSecretStore

class AppContainer(context: Context) {
    private val appContext = context.applicationContext
    private val applicationScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    val secureSecretStore: AndroidKeystoreSecretStore = AndroidKeystoreSecretStore()
    val appManager: AppManager

    init {
        appManager =
            AppManager(
                context = appContext,
                applicationScope = applicationScope,
                secureSecretStore = secureSecretStore,
            )
    }
}
