package com.underground.app.ui.wall

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.CreatePostReq
import com.underground.app.data.PostDto
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** Wall of one user: posts + like toggling + (own wall) composer and delete. */
class WallViewModel : ViewModel() {
    private val api = UndergroundApp.container.userApi

    private val _posts = MutableStateFlow<List<PostDto>>(emptyList())
    val posts: StateFlow<List<PostDto>> = _posts.asStateFlow()

    private val _busy = MutableStateFlow(false)
    val busy: StateFlow<Boolean> = _busy.asStateFlow()

    private var userId: String? = null

    fun load(userId: String) {
        this.userId = userId
        viewModelScope.launch {
            try {
                _posts.value = api.userPosts(userId)
            } catch (e: Exception) {
                // The wall is auxiliary — keep the previous state on failure.
            }
        }
    }

    fun refresh() = userId?.let(::load)

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

    fun createPost(text: String, onDone: () -> Unit) {
        viewModelScope.launch {
            _busy.value = true
            try {
                api.createPost(CreatePostReq(text = text.trim()))
                refresh()
                onDone()
            } catch (e: Exception) {
                // Composer stays filled so the user can retry.
            } finally {
                _busy.value = false
            }
        }
    }

    fun deletePost(post: PostDto) {
        viewModelScope.launch {
            try {
                api.deletePost(post.id)
                _posts.update { list -> list.filterNot { it.id == post.id } }
            } catch (e: Exception) {
                // Keep the post visible on failure.
            }
        }
    }
}
