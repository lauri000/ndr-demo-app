package social.innode.ndr.demo

import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import social.innode.ndr.demo.core.AppContainer
import social.innode.ndr.demo.ui.navigation.NdrApp
import social.innode.ndr.demo.ui.theme.IrisChatTheme

class MainActivity : ComponentActivity() {
    private lateinit var container: AppContainer

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Log.d(TAG, "onCreate")
        container = (application as IrisChatApp).container

        setContent {
            IrisChatTheme {
                NdrApp(container = container)
            }
        }
    }

    override fun onStart() {
        super.onStart()
        Log.d(TAG, "onStart")
        container.appManager.appForegrounded()
    }

    override fun onStop() {
        Log.d(TAG, "onStop")
        super.onStop()
    }

    private companion object {
        const val TAG = "NdrDebug"
    }
}
