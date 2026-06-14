package com.underground.app.ui.wall

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.CommentDto
import com.underground.app.data.CreateCommentReq
import com.underground.app.ui.theme.Muted
import com.underground.app.util.Format
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class CommentsViewModel : ViewModel() {
    private val api = UndergroundApp.container.userApi

    private val _comments = MutableStateFlow<List<CommentDto>>(emptyList())
    val comments: StateFlow<List<CommentDto>> = _comments.asStateFlow()

    private val _busy = MutableStateFlow(false)
    val busy: StateFlow<Boolean> = _busy.asStateFlow()

    fun load(postId: String) {
        viewModelScope.launch {
            try {
                _comments.value = api.comments(postId)
            } catch (e: Exception) {
                // Keep previous state.
            }
        }
    }

    fun add(postId: String, text: String, onDone: () -> Unit) {
        viewModelScope.launch {
            _busy.value = true
            try {
                api.addComment(postId, CreateCommentReq(text.trim()))
                load(postId)
                onDone()
            } catch (e: Exception) {
                // Input stays for retry.
            } finally {
                _busy.value = false
            }
        }
    }
}

@Composable
fun CommentsScreen(vm: CommentsViewModel, postId: String, onBack: () -> Unit) {
    LaunchedEffect(postId) { vm.load(postId) }
    val comments by vm.comments.collectAsState()
    val busy by vm.busy.collectAsState()
    var text by remember { mutableStateOf("") }

    Column(Modifier.fillMaxSize().imePadding()) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(4.dp)) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
            }
            Text("Коментарі", style = MaterialTheme.typography.titleMedium)
        }
        LazyColumn(
            modifier = Modifier.weight(1f),
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            if (comments.isEmpty()) {
                item { Text("Ще немає коментарів — будь першим!", color = Muted) }
            }
            items(comments, key = { it.id }) { comment ->
                Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface)) {
                    Column(Modifier.fillMaxWidth().padding(10.dp)) {
                        Row {
                            Text(comment.authorName, fontWeight = FontWeight.SemiBold)
                            Spacer(Modifier.width(8.dp))
                            Text(Format.eventDate(comment.createdAt), color = Muted, style = MaterialTheme.typography.labelSmall)
                        }
                        Text(comment.text, style = MaterialTheme.typography.bodyMedium)
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
                placeholder = { Text("Ваш коментар…") },
                modifier = Modifier.weight(1f),
            )
            IconButton(
                onClick = { vm.add(postId, text) { text = "" } },
                enabled = text.isNotBlank() && !busy,
            ) {
                Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Надіслати")
            }
        }
    }
}
