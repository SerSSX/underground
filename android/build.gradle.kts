plugins {
    id("com.android.application") version "8.5.2" apply false
    id("org.jetbrains.kotlin.android") version "2.0.20" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.0.20" apply false
    id("org.jetbrains.kotlin.plugin.serialization") version "2.0.20" apply false
    // Firebase Cloud Messaging (TASK-017 M4). Applied by the app module only when
    // google-services.json is present, so builds work without Firebase credentials.
    id("com.google.gms.google-services") version "4.4.2" apply false
}
