package com.underground.app.ui.chat

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.background
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material3.Badge
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.ConversationDto
import com.underground.app.ui.theme.Accent
import com.underground.app.ui.theme.Muted
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class ConversationsViewModel : ViewModel() {
    private val api = UndergroundApp.container.userApi

    private val _items = MutableStateFlow<List<ConversationDto>>(emptyList())
    val items: StateFlow<List<ConversationDto>> = _items.asStateFlow()

    private val _loaded = MutableStateFlow(false)
    val loaded: StateFlow<Boolean> = _loaded.asStateFlow()

    fun load() {
        viewModelScope.launch {
            try {
                _items.value = api.conversations()
            } catch (e: Exception) {
                // Keep previous list.
            } finally {
                _loaded.value = true
            }
        }
    }
}

/** Conversation list (embedded in the Вхідні tab). */
@Composable
fun ConversationsList(
    vm: ConversationsViewModel,
    onOpenChat: (peerId: String, peerName: String) -> Unit,
) {
    LaunchedEffect(Unit) { vm.load() }
    val items by vm.items.collectAsState()
    val loaded by vm.loaded.collectAsState()

    Column(Modifier.fillMaxSize()) {
        if (loaded && items.isEmpty()) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text("Ще немає діалогів.\nНапишіть організатору з картки події!", color = Muted)
            }
        } else {
            LazyColumn(contentPadding = PaddingValues(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                items(items, key = { it.peerId }) { conv ->
                    Card(
                        onClick = { onOpenChat(conv.peerId, conv.peerName) },
                        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
                    ) {
                        Row(
                            Modifier.fillMaxWidth().padding(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Box(
                                modifier = Modifier.size(40.dp).clip(CircleShape).background(Accent),
                                contentAlignment = Alignment.Center,
                            ) {
                                Text(conv.peerName.take(1).uppercase(), color = Color.White, fontWeight = FontWeight.Bold)
                            }
                            Spacer(Modifier.width(12.dp))
                            Column(Modifier.weight(1f)) {
                                Text(conv.peerName, fontWeight = FontWeight.SemiBold)
                                Text(
                                    (if (conv.lastFromMe) "Ви: " else "") + conv.lastText,
                                    color = Muted,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    style = MaterialTheme.typography.bodySmall,
                                )
                            }
                            if (conv.unread > 0) {
                                Badge { Text("${conv.unread}") }
                            }
                        }
                    }
                }
            }
        }
    }
}
