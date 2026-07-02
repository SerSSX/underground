package com.underground.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.core.app.ActivityCompat
import androidx.lifecycle.viewmodel.compose.viewModel
import com.underground.app.push.AppNotifications
import com.underground.app.push.PushTokens
import com.underground.app.ui.AppScaffold
import com.underground.app.ui.auth.AuthScreen
import com.underground.app.ui.auth.AuthState
import com.underground.app.ui.auth.AuthViewModel
import com.underground.app.ui.gift.ClaimGiftScreen
import com.underground.app.ui.onboarding.OnboardingScreen
import com.underground.app.ui.staff.AcceptStaffScreen
import com.underground.app.ui.staff.StaffInvite
import com.underground.app.ui.theme.UndergroundTheme

class MainActivity : ComponentActivity() {
    private val notifPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { /* result ignored */ }

    // A gift claim token from a deep link (underground://g/<token> or .../g/<token>), if any.
    private val giftToken = mutableStateOf<String?>(null)

    // A door-staff invite from a deep link (underground://staff/<token>?e=&t=), if any.
    private val staffInvite = mutableStateOf<StaffInvite?>(null)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        AppNotifications.ensureChannel(this)
        maybeRequestNotificationPermission()
        giftToken.value = extractGiftToken(intent)
        staffInvite.value = extractStaffInvite(intent)
        setContent {
            UndergroundTheme {
                Root(
                    giftToken = giftToken.value,
                    staffInvite = staffInvite.value,
                    onGiftHandled = { giftToken.value = null },
                    onStaffHandled = { staffInvite.value = null },
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        extractGiftToken(intent)?.let { giftToken.value = it }
        extractStaffInvite(intent)?.let { staffInvite.value = it }
    }

    /** Pull a gift claim token from `underground://g/<token>` or `https://host/g/<token>`. */
    private fun extractGiftToken(intent: Intent?): String? {
        val data: Uri = intent?.data ?: return null
        val segments = data.pathSegments ?: emptyList()
        return when {
            data.scheme == "underground" && data.host == "g" -> segments.firstOrNull()
            segments.size >= 2 && segments[0] == "g" -> segments[1]
            else -> null
        }
    }

    /** Pull a door-staff invite from `underground://staff/<token>?e=<eventId>&t=<title>`. */
    private fun extractStaffInvite(intent: Intent?): StaffInvite? {
        val data: Uri = intent?.data ?: return null
        if (data.scheme != "underground" || data.host != "staff") return null
        val token = data.pathSegments?.firstOrNull() ?: return null
        return StaffInvite(token, data.getQueryParameter("e"), data.getQueryParameter("t"))
    }

    /** Android 13+ requires a runtime grant to post notifications. */
    private fun maybeRequestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            ActivityCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) !=
            PackageManager.PERMISSION_GRANTED
        ) {
            notifPermission.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }
}

@Composable
private fun Root(
    giftToken: String?,
    staffInvite: StaffInvite?,
    onGiftHandled: () -> Unit,
    onStaffHandled: () -> Unit,
    vm: AuthViewModel = viewModel(),
) {
    if (giftToken != null) {
        // Deep-link gift claim takes over regardless of session (browse-before-auth).
        ClaimGiftScreen(token = giftToken, onDone = { onGiftHandled(); vm.refresh() })
        return
    }
    val state by vm.state.collectAsState()
    if (staffInvite != null) {
        // Accepting a controller invite requires an account — sign in first, then accept.
        when (state) {
            AuthState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            AuthState.LoggedOut -> AuthScreen(vm)
            is AuthState.LoggedIn ->
                AcceptStaffScreen(invite = staffInvite, onDone = { onStaffHandled(); vm.refresh() })
        }
        return
    }
    when (val s = state) {
        AuthState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            CircularProgressIndicator()
        }
        AuthState.LoggedOut -> AuthScreen(vm)
        is AuthState.LoggedIn ->
            if (s.needsOnboarding) {
                val busy by vm.busy.collectAsState()
                OnboardingScreen(busy = busy, onDone = vm::completeOnboarding)
            } else {
                val context = LocalContext.current
                // Register this device's push token once we know who is signed in.
                LaunchedEffect(s.user.id) { PushTokens.register(context) }
                AppScaffold(user = s.user, onLogout = vm::logout, onUserUpdated = vm::updateUser)
            }
    }
}
