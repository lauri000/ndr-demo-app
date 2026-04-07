package social.innode.ndr.demo

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import social.innode.ndr.demo.ui.navigation.NdrApp
import social.innode.ndr.demo.ui.theme.NdrDemoTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val container = (application as NdrDemoApp).container

        setContent {
            NdrDemoTheme {
                NdrApp(container = container)
            }
        }
    }
}
