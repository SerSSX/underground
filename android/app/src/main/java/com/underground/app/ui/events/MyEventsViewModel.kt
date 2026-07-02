package com.underground.app.ui.events

import android.net.Uri
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
import retrofit2.HttpException

class MyEventsViewModel : ViewModel() {
    private val api = UndergroundApp.container.eventApi

    private val _events = MutableStateFlow<List<EventDto>>(emptyList())
    val events: StateFlow<List<EventDto>> = _events.asStateFlow()

    // Events the user may scan but does NOT own (delegated door-controller role, TASK-118-E).
    private val _delegatedEvents = MutableStateFlow<List<EventDto>>(emptyList())
    val delegatedEvents: StateFlow<List<EventDto>> = _delegatedEvents.asStateFlow()

    private val _loading = MutableStateFlow(true)
    val loading: StateFlow<Boolean> = _loading.asStateFlow()

    init {
        load()
    }

    fun load() {
        viewModelScope.launch {
            _loading.value = true
            val owned = try {
                api.myEvents()
            } catch (e: Exception) {
                emptyList()
            }
            val scannable = try {
                api.myStaffEvents()
            } catch (e: Exception) {
                emptyList()
            }
            val ownedIds = owned.map { it.id }.toSet()
            _events.value = owned
            _delegatedEvents.value = scannable.filter { it.id !in ownedIds }
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

    /** Mint a single-use door-controller invite for [eventId]. The deep-link carries the event id
     *  (and title) so the controller's app can scan straight away without an extra round-trip. */
    fun inviteStaff(eventId: String, eventTitle: String, onDone: (String) -> Unit, onError: (String) -> Unit) {
        viewModelScope.launch {
            try {
                val resp = api.inviteStaff(eventId)
                onDone("underground://staff/${resp.inviteToken}?e=$eventId&t=${Uri.encode(eventTitle)}")
            } catch (e: HttpException) {
                onError(if (e.code() == 403) "Лише організатор може запрошувати" else "Помилка ${e.code()}")
            } catch (e: Exception) {
                onError(e.message ?: "Помилка мережі")
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
