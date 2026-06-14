package com.underground.app.ui.feed

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ViewDay
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
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.PostDto
import com.underground.app.ui.components.PostCard
import com.underground.app.ui.theme.Muted
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

class FeedViewModel : ViewModel() {
    private val api = UndergroundApp.container.userApi

    private val _posts = MutableStateFlow<List<PostDto>>(emptyList())
    val posts: StateFlow<List<PostDto>> = _posts.asStateFlow()

    private val _loading = MutableStateFlow(true)
    val loading: StateFlow<Boolean> = _loading.asStateFlow()

    fun load() {
        viewModelScope.launch {
            try {
                _posts.value = api.feed()
            } catch (e: Exception) {
                // Keep the previous feed on failure.
            } finally {
                _loading.value = false
            }
        }
    }

    fun toggleLike(post: PostDto) {
        val liked = !post.likedByMe
        _posts.update { list ->
            list.map {
                if (it.id == post.id) {
                    it.copy(likedByMe = liked, likesCount = it.likesCount + if (liked) 1 else -1)
                } else {
                    it
                }
            }
        }
        viewModelScope.launch {
            try {
                if (liked) api.likePost(post.id) else api.unlikePost(post.id)
            } catch (e: Exception) {
                _posts.update { list -> list.map { if (it.id == post.id) post else it } }
            }
        }
    }
}

/** Activity feed: posts from followed users + posts about events in my interest categories. */
@Composable
fun FeedScreen(
    vm: FeedViewModel,
    currentUserId: String,
    onOpenEvent: (String) -> Unit,
    onOpenAuthor: (String) -> Unit,
    onOpenComments: (String) -> Unit,
) {
    LaunchedEffect(Unit) { vm.load() }
    val posts by vm.posts.collectAsState()
    val loading by vm.loading.collectAsState()

    when {
        loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            CircularProgressIndicator()
        }
        posts.isEmpty() -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.padding(32.dp)) {
                Icon(Icons.Filled.ViewDay, contentDescription = null, tint = Muted, modifier = Modifier.size(48.dp))
                Spacer(Modifier.height(8.dp))
                Text("Стрічка порожня", style = MaterialTheme.typography.titleMedium)
                Text(
                    "Підпишись на організаторів чи друзів у їхніх профілях — " +
                        "і обери інтереси, щоб бачити відгуки про події твоїх категорій.",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Muted,
                    textAlign = TextAlign.Center,
                )
            }
        }
        else -> LazyColumn(
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            modifier = Modifier.fillMaxSize(),
        ) {
            item {
                Text("Стрічка", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
            }
            items(posts, key = { it.id }) { post ->
                PostCard(
                    post = post,
                    isMine = post.authorId == currentUserId,
                    onToggleLike = { vm.toggleLike(post) },
                    onOpenComments = { onOpenComments(post.id) },
                    onOpenEvent = onOpenEvent,
                    onOpenAuthor = onOpenAuthor,
                )
            }
        }
    }
}
