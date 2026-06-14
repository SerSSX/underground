package com.underground.app.push

import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage

/** Receives FCM pushes and posts a system notification (TASK-017 M4). Dormant until
 *  Firebase is configured (google-services.json present). */
class UndergroundMessagingService : FirebaseMessagingService() {
    override fun onNewToken(token: String) {
        PushTokens.upload(token)
    }

    override fun onMessageReceived(message: RemoteMessage) {
        val n = message.notification
        val title = n?.title ?: message.data["title"] ?: "Underground"
        val body = n?.body ?: message.data["body"]
        AppNotifications.show(this, title, body)
    }
}
