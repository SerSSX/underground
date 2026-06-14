package com.underground.app.ui.detail

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.EventDto
import com.underground.app.data.EventSocialDto
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** Loads social proof + similar events for the detail screen (TASK-017 M3). */
class EventExtrasViewModel : ViewModel() {
    private val api = UndergroundApp.container.eventApi

    private val _social = MutableStateFlow<EventSocialDto?>(null)
    val social: StateFlow<EventSocialDto?> = _social.asStateFlow()

    private val _similar = MutableStateFlow<List<EventDto>>(emptyList())
    val similar: StateFlow<List<EventDto>> = _similar.asStateFlow()

    /** Fallback event when it wasn't already in a list (deep links, similar-event taps). */
    private val _event = MutableStateFlow<EventDto?>(null)
    val event: StateFlow<EventDto?> = _event.asStateFlow()

    fun load(eventId: String) {
        viewModelScope.launch {
            try {
                _event.value = api.getEvent(eventId)
            } catch (e: Exception) {
                // may already be supplied by the caller
            }
            try {
                _social.value = api.social(eventId)
            } catch (e: Exception) {
                // auxiliary — keep the page usable without it
            }
            try {
                _similar.value = api.similar(eventId)
            } catch (e: Exception) {
                // auxiliary
            }
        }
    }
}
