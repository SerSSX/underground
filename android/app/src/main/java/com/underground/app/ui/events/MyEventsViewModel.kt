package com.underground.app.ui.events

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.EventDto
import com.underground.app.data.UpdateEventReq
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

class MyEventsViewModel : ViewModel() {
    private val api = UndergroundApp.container.eventApi

    private val _events = MutableStateFlow<List<EventDto>>(emptyList())
    val events: StateFlow<List<EventDto>> = _events.asStateFlow()

    private val _loading = MutableStateFlow(true)
    val loading: StateFlow<Boolean> = _loading.asStateFlow()

    init {
        load()
    }

    fun load() {
        viewModelScope.launch {
            _loading.value = true
            _events.value = try {
                api.myEvents()
            } catch (e: Exception) {
                emptyList()
            }
            _loading.value = false
        }
    }

    fun find(id: String): EventDto? = _events.value.find { it.id == id }

    /** Organizer admin action; the updated event replaces the stale row in place. */
    fun update(id: String, req: UpdateEventReq, onError: (String) -> Unit = {}) {
        viewModelScope.launch {
            try {
                val updated = api.update(id, req)
                _events.update { list -> list.map { if (it.id == id) updated else it } }
            } catch (e: Exception) {
                onError("Не вдалося оновити подію")
            }
        }
    }

    fun cancel(id: String, onError: (String) -> Unit = {}) {
        viewModelScope.launch {
            try {
                api.cancel(id)
                load()
            } catch (e: Exception) {
                onError("Не вдалося скасувати подію")
            }
        }
    }
}
