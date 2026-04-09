package social.innode.ndr.demo

import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import social.innode.ndr.demo.ui.navigation.NdrApp
import social.innode.ndr.demo.ui.theme.NdrDemoTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Log.d(TAG, "onCreate")
        val container = (application as NdrDemoApp).container

        setContent {
            NdrDemoTheme {
                NdrApp(container = container)
            }
        }
    }

    override fun onStart() {
        super.onStart()
        Log.d(TAG, "onStart")
    }

    override fun onStop() {
        Log.d(TAG, "onStop")
        super.onStop()
    }

    private companion object {
        const val TAG = "NdrDebug"
    }
}
