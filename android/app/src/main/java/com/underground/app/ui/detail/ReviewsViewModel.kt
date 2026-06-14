package com.underground.app.ui.detail

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.EventReviewsDto
import com.underground.app.data.PostReviewReq
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class ReviewsUiState(
    val data: EventReviewsDto = EventReviewsDto(),
    val sending: Boolean = false,
    val error: String? = null,
)

class ReviewsViewModel : ViewModel() {
    private val api = UndergroundApp.container.eventApi

    private val _state = MutableStateFlow(ReviewsUiState())
    val state: StateFlow<ReviewsUiState> = _state.asStateFlow()

    fun load(eventId: String) {
        viewModelScope.launch {
            try {
                _state.update { it.copy(data = api.reviews(eventId), error = null) }
            } catch (e: Exception) {
                // Reviews are auxiliary on the detail screen — fail quietly.
            }
        }
    }

    fun post(eventId: String, rating: Int, text: String, onDone: () -> Unit) {
        viewModelScope.launch {
            _state.update { it.copy(sending = true, error = null) }
            try {
                api.postReview(PostReviewReq(eventId, rating, text.trim().ifEmpty { null }))
                _state.update { it.copy(data = api.reviews(eventId), sending = false) }
                onDone()
            } catch (e: Exception) {
                _state.update { it.copy(sending = false, error = "Не вдалося надіслати відгук") }
            }
        }
    }
}
