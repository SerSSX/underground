package com.underground.app

import android.Manifest
import android.content.pm.PackageManager
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
import com.underground.app.ui.onboarding.OnboardingScreen
import com.underground.app.ui.theme.UndergroundTheme

class MainActivity : ComponentActivity() {
    private val notifPermission =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { /* result ignored */ }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        AppNotifications.ensureChannel(this)
        maybeRequestNotificationPermission()
        setContent {
            UndergroundTheme {
                Root()
            }
        }
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
private fun Root(vm: AuthViewModel = viewModel()) {
    val state by vm.state.collectAsState()
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
