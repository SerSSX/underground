package com.underground.app.ui.chat

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.MessageDto
import com.underground.app.data.SendMessageReq
import com.underground.app.ui.theme.Accent
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class ChatViewModel : ViewModel() {
    private val api = UndergroundApp.container.userApi

    private val _messages = MutableStateFlow<List<MessageDto>>(emptyList())
    val messages: StateFlow<List<MessageDto>> = _messages.asStateFlow()

    private val _busy = MutableStateFlow(false)
    val busy: StateFlow<Boolean> = _busy.asStateFlow()

    fun load(peerId: String) {
        viewModelScope.launch {
            try {
                _messages.value = api.thread(peerId)
            } catch (e: Exception) {
                // Keep previous thread.
            }
        }
    }

    fun send(peerId: String, text: String, onDone: () -> Unit) {
        viewModelScope.launch {
            _busy.value = true
            try {
                api.sendMessage(SendMessageReq(peerId, text.trim()))
                load(peerId)
                onDone()
            } catch (e: Exception) {
                // Input stays for retry.
            } finally {
                _busy.value = false
            }
        }
    }
}

/** Thread with one peer; polls every few seconds while open. */
@Composable
fun ChatScreen(
    vm: ChatViewModel,
    peerId: String,
    peerName: String,
    currentUserId: String,
    onBack: () -> Unit,
) {
    LaunchedEffect(peerId) {
        while (true) {
            vm.load(peerId)
            delay(4000)
        }
    }
    val messages by vm.messages.collectAsState()
    val busy by vm.busy.collectAsState()
    var text by remember { mutableStateOf("") }
    val listState = rememberLazyListState()

    LaunchedEffect(messages.size) {
        if (messages.isNotEmpty()) listState.animateScrollToItem(messages.size - 1)
    }

    Column(Modifier.fillMaxSize().imePadding()) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(4.dp)) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
            }
            Text(peerName, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
        }
        LazyColumn(
            state = listState,
            modifier = Modifier.weight(1f),
            contentPadding = PaddingValues(12.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            items(messages, key = { it.id }) { message ->
                val mine = message.senderId == currentUserId
                Box(Modifier.fillMaxWidth(), contentAlignment = if (mine) Alignment.CenterEnd else Alignment.CenterStart) {
                    Box(
                        Modifier
                            .widthIn(max = 280.dp)
                            .clip(RoundedCornerShape(14.dp))
                            .background(if (mine) Accent else MaterialTheme.colorScheme.surfaceVariant)
                            .padding(horizontal = 12.dp, vertical = 8.dp),
                    ) {
                        Text(
                            message.text,
                            color = if (mine) Color.White else MaterialTheme.colorScheme.onSurface,
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                }
            }
        }
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                placeholder = { Text("Повідомлення…") },
                modifier = Modifier.weight(1f),
            )
            IconButton(
                onClick = { vm.send(peerId, text) { text = "" } },
                enabled = text.isNotBlank() && !busy,
            ) {
                Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Надіслати", tint = Accent)
            }
        }
    }
}
