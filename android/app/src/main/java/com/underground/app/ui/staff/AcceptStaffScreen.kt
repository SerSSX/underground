package com.underground.app.ui.staff

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.underground.app.ui.events.EventScanButton
import com.underground.app.ui.theme.Muted

/** Landing when a controller opens a door-staff invite deep link: accept, then scan the event. */
@Composable
fun AcceptStaffScreen(invite: StaffInvite, onDone: () -> Unit, vm: AcceptStaffViewModel = viewModel()) {
    LaunchedEffect(invite.token) { vm.accept(invite.token) }
    val state by vm.state.collectAsState()

    Box(Modifier.fillMaxSize().padding(24.dp), contentAlignment = Alignment.Center) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            when (val s = state) {
                AcceptState.Accepting -> {
                    CircularProgressIndicator()
                    Text("Приймаємо запрошення…", color = Muted)
                }

                AcceptState.Accepted -> {
                    Text("✓ Ти контролер", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
                    invite.eventTitle?.let {
                        Text(it, style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
                    }
                    Text(
                        "Скануй QR квитків на вході. Доступ лише до цієї події.",
                        color = Muted,
                        textAlign = TextAlign.Center,
                    )
                    if (invite.eventId != null) {
                        EventScanButton(eventId = invite.eventId, modifier = Modifier.fillMaxWidth())
                    }
                    OutlinedButton(onClick = onDone, modifier = Modifier.fillMaxWidth()) { Text("Готово") }
                }

                is AcceptState.Failed -> {
                    Text("Не вдалося", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
                    Text(s.message, color = Muted, textAlign = TextAlign.Center)
                    OutlinedButton(onClick = onDone, modifier = Modifier.fillMaxWidth()) { Text("Закрити") }
                }
            }
        }
    }
}
