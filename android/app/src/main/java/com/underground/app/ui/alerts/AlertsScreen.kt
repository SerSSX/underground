package com.underground.app.ui.alerts

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.NotificationsNone
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.NotificationDto
import com.underground.app.ui.theme.Accent
import com.underground.app.ui.theme.Muted
import com.underground.app.util.Format
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class AlertsUiState(
    val items: List<NotificationDto> = emptyList(),
    val loading: Boolean = true,
)

class AlertsViewModel : ViewModel() {
    private val api = UndergroundApp.container.userApi

    private val _state = MutableStateFlow(AlertsUiState())
    val state: StateFlow<AlertsUiState> = _state.asStateFlow()

    /** Loads the list, then marks everything read (the unread accents stay until reload). */
    fun load() {
        viewModelScope.launch {
            try {
                val items = api.notifications()
                _state.value = AlertsUiState(items = items, loading = false)
                if (items.any { !it.read }) api.markNotificationsRead()
            } catch (e: Exception) {
                _state.value = _state.value.copy(loading = false)
            }
        }
    }
}

/** Notifications tab (F-14b): tap on an event notification opens the event. */
@Composable
fun AlertsScreen(vm: AlertsViewModel, onOpenEvent: (String) -> Unit) {
    LaunchedEffect(Unit) { vm.load() }
    val state by vm.state.collectAsState()

    when {
        state.loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            CircularProgressIndicator()
        }
        state.items.isEmpty() -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Column(horizontalAlignment = Alignment.CenterHorizontally) {
                Icon(
                    Icons.Filled.NotificationsNone,
                    contentDescription = null,
                    tint = Muted,
                    modifier = Modifier.size(48.dp),
                )
                Spacer(Modifier.height(8.dp))
                Text("Поки тихо", style = MaterialTheme.typography.titleMedium)
                Text(
                    "Обери інтереси в профілі — і ми підкажемо нові події",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Muted,
                )
            }
        }
        else -> LazyColumn(
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(10.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            items(state.items, key = { it.id }) { item ->
                NotificationCard(item) { item.eventId?.let(onOpenEvent) }
            }
        }
    }
}

@Composable
private fun NotificationCard(item: NotificationDto, onClick: () -> Unit) {
    Card(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
    ) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    item.title,
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = if (item.read) FontWeight.Normal else FontWeight.Bold,
                    color = if (item.read) MaterialTheme.colorScheme.onSurface else Accent,
                    modifier = Modifier.weight(1f),
                )
            }
            item.body?.let { Text(it, style = MaterialTheme.typography.bodyMedium, color = Muted) }
            Text(Format.eventDate(item.createdAt), style = MaterialTheme.typography.labelSmall, color = Muted)
        }
    }
}
