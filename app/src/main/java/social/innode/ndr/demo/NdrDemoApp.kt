package social.innode.ndr.demo

import android.app.Application
import social.innode.ndr.demo.core.AppContainer

class NdrDemoApp : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        container = AppContainer(this)
    }
}
