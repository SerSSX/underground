package com.underground.app.ui.profile

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.EventDto
import com.underground.app.data.PublicProfileDto
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class PublicProfileUiState(
    val profile: PublicProfileDto? = null,
    val events: List<EventDto> = emptyList(),
    val loading: Boolean = true,
    val error: String? = null,
)

class PublicProfileViewModel : ViewModel() {
    private val api = UndergroundApp.container.userApi

    private val _state = MutableStateFlow(PublicProfileUiState())
    val state: StateFlow<PublicProfileUiState> = _state.asStateFlow()

    fun load(userId: String) {
        viewModelScope.launch {
            _state.update { it.copy(loading = true, error = null) }
            try {
                val profile = api.profile(userId)
                val events = api.userEvents(userId)
                _state.value = PublicProfileUiState(profile = profile, events = events, loading = false)
            } catch (e: Exception) {
                _state.update { it.copy(loading = false, error = "Не вдалося завантажити профіль") }
            }
        }
    }

    fun toggleFollow() {
        val profile = _state.value.profile ?: return
        val nowFollowing = !profile.isFollowing
        // Optimistic flip; reverted on failure.
        _state.update {
            it.copy(
                profile = profile.copy(
                    isFollowing = nowFollowing,
                    followersCount = profile.followersCount + if (nowFollowing) 1 else -1,
                ),
            )
        }
        viewModelScope.launch {
            try {
                if (nowFollowing) api.follow(profile.id) else api.unfollow(profile.id)
            } catch (e: Exception) {
                _state.update { it.copy(profile = profile) }
            }
        }
    }
}
