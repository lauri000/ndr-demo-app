package social.innode.ndr.demo.push

import android.util.Log
import com.google.firebase.messaging.FirebaseMessagingService

class IrisFirebaseMessagingService : FirebaseMessagingService() {
    override fun onNewToken(token: String) {
        Log.d(TAG, "FCM token refreshed")
    }

    private companion object {
        const val TAG = "IrisPush"
    }
}
