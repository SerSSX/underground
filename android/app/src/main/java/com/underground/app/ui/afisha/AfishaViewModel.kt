package com.underground.app.ui.afisha

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.data.DiscoverSectionDto
import com.underground.app.data.EventDto
import com.underground.app.data.EventRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

sealed interface AfishaUiState {
    data object Loading : AfishaUiState
    data object Empty : AfishaUiState
    data class Success(val events: List<EventDto>) : AfishaUiState
    data class Sections(val sections: List<DiscoverSectionDto>) : AfishaUiState
    data class Error(val message: String) : AfishaUiState
}

/** Discovery first (personalized "For You"), catalog second. */
enum class AfishaMode { ForYou, All }

/** Pure reducer for the loaded list → testable without coroutines. */
internal fun reduce(events: List<EventDto>): AfishaUiState =
    if (events.isEmpty()) AfishaUiState.Empty else AfishaUiState.Success(events)

/** Pure reducer for the sectioned "For You" feed (TASK-200). */
internal fun reduceSections(sections: List<DiscoverSectionDto>): AfishaUiState =
    if (sections.isEmpty()) AfishaUiState.Empty else AfishaUiState.Sections(sections)

/** Pure category filter (null = all). */
internal fun filterByCategory(events: List<EventDto>, category: String?): List<EventDto> =
    if (category == null) events else events.filter { it.category == category }

class AfishaViewModel : ViewModel() {
    private val repo = EventRepository()

    private val _state = MutableStateFlow<AfishaUiState>(AfishaUiState.Loading)
    val state: StateFlow<AfishaUiState> = _state.asStateFlow()

    private val _category = MutableStateFlow<String?>(null)
    val category: StateFlow<String?> = _category.asStateFlow()

    private val _mode = MutableStateFlow(AfishaMode.ForYou)
    val mode: StateFlow<AfishaMode> = _mode.asStateFlow()

    private val _savedIds = MutableStateFlow<Set<String>>(emptySet())
    val savedIds: StateFlow<Set<String>> = _savedIds.asStateFlow()

    init {
        load()
        refreshSaved()
    }

    fun load() {
        _state.value = AfishaUiState.Loading
        viewModelScope.launch {
            _state.value = try {
                if (_mode.value == AfishaMode.ForYou) reduceSections(repo.discoverSections())
                else reduce(repo.published())
            } catch (e: Exception) {
                AfishaUiState.Error(e.message ?: "Network error")
            }
        }
    }

    fun setMode(mode: AfishaMode) {
        if (_mode.value == mode) return
        _mode.value = mode
        _category.value = null
        load()
    }

    private fun refreshSaved() {
        viewModelScope.launch {
            try {
                _savedIds.value = repo.savedIds()
            } catch (e: Exception) {
                // best-effort; stars just default to un-saved
            }
        }
    }

    /** Optimistic save toggle; reverted on failure. */
    fun toggleSave(id: String) {
        val nowSaved = id !in _savedIds.value
        _savedIds.update { if (nowSaved) it + id else it - id }
        viewModelScope.launch {
            try {
                if (nowSaved) repo.save(id) else repo.unsave(id)
            } catch (e: Exception) {
                _savedIds.update { if (nowSaved) it - id else it + id }
            }
        }
    }

    fun selectCategory(category: String?) {
        _category.value = category
    }

    fun find(id: String): EventDto? = when (val s = state.value) {
        is AfishaUiState.Success -> s.events.firstOrNull { it.id == id }
        is AfishaUiState.Sections -> s.sections.flatMap { it.events }.firstOrNull { it.id == id }
        else -> null
    }
}
