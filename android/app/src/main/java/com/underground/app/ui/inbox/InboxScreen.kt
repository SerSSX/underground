package com.underground.app.ui.inbox

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.UnreadDto
import com.underground.app.ui.alerts.AlertsScreen
import com.underground.app.ui.alerts.AlertsViewModel
import com.underground.app.ui.chat.ConversationsList
import com.underground.app.ui.chat.ConversationsViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** Badge source for the Вхідні tab (polled by the scaffold on navigation). */
class UnreadViewModel : ViewModel() {
    private val api = UndergroundApp.container.userApi

    private val _unread = MutableStateFlow(UnreadDto())
    val unread: StateFlow<UnreadDto> = _unread.asStateFlow()

    fun refresh() {
        viewModelScope.launch {
            try {
                _unread.value = api.unread()
            } catch (e: Exception) {
                // Badge is best-effort.
            }
        }
    }
}

/** Вхідні: Сповіщення | Повідомлення під одним табом (P0-1/P0-5 з аудиту). */
@Composable
fun InboxScreen(
    alertsVm: AlertsViewModel,
    conversationsVm: ConversationsViewModel,
    unread: UnreadDto,
    onOpenEvent: (String) -> Unit,
    onOpenChat: (peerId: String, peerName: String) -> Unit,
) {
    var tab by remember { mutableIntStateOf(if (unread.messages > unread.notifications) 1 else 0) }

    Column(Modifier.fillMaxSize()) {
        TabRow(selectedTabIndex = tab) {
            Tab(
                selected = tab == 0,
                onClick = { tab = 0 },
                text = { TabLabel("Сповіщення", unread.notifications) },
            )
            Tab(
                selected = tab == 1,
                onClick = { tab = 1 },
                text = { TabLabel("Повідомлення", unread.messages) },
            )
        }
        when (tab) {
            0 -> AlertsScreen(alertsVm, onOpenEvent)
            else -> ConversationsList(conversationsVm, onOpenChat)
        }
    }
}

@Composable
private fun TabLabel(title: String, count: Long) {
    if (count > 0) {
        BadgedBox(badge = { Badge { Text("$count") } }) { Text(title) }
    } else {
        Text(title)
    }
}
