package com.underground.app.push

import android.content.Context
import android.util.Log
import com.google.firebase.FirebaseApp
import com.google.firebase.messaging.FirebaseMessaging
import com.underground.app.UndergroundApp
import com.underground.app.data.RegisterDeviceReq
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

/** Retrieves the FCM token and registers it with the backend (TASK-017 M4). */
object PushTokens {
    private const val TAG = "PushTokens"

    /** Fetch the current token and upload it. Safe no-op when Firebase is not
     *  configured (google-services.json absent → no default FirebaseApp). */
    fun register(context: Context) {
        if (FirebaseApp.getApps(context).isEmpty()) {
            Log.d(TAG, "Firebase not configured — skipping push registration")
            return
        }
        FirebaseMessaging.getInstance().token
            .addOnSuccessListener { token -> upload(token) }
            .addOnFailureListener { e -> Log.w(TAG, "FCM token fetch failed", e) }
    }

    /** Upload a token (also called from onNewToken). Best-effort; a logged-out caller
     *  simply gets a 401 that we swallow. */
    fun upload(token: String) {
        CoroutineScope(Dispatchers.IO).launch {
            try {
                UndergroundApp.container.userApi.registerDevice(RegisterDeviceReq(token))
            } catch (e: Exception) {
                Log.d(TAG, "device registration skipped: ${e.message}")
            }
        }
    }
}
