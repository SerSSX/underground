package com.underground.app.ui.onboarding

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.PrefsDto
import com.underground.app.data.PutPrefsReq
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

class InterestsViewModel : ViewModel() {
    private val api = UndergroundApp.container.userApi

    private val _prefs = MutableStateFlow<PrefsDto?>(null)
    val prefs: StateFlow<PrefsDto?> = _prefs.asStateFlow()

    private val _busy = MutableStateFlow(false)
    val busy: StateFlow<Boolean> = _busy.asStateFlow()

    init {
        viewModelScope.launch {
            _prefs.value = try {
                api.getPrefs()
            } catch (e: Exception) {
                PrefsDto()
            }
        }
    }

    fun save(categories: List<String>, city: String?, onDone: () -> Unit) {
        viewModelScope.launch {
            _busy.value = true
            try {
                api.putPrefs(PutPrefsReq(categories, city?.takeIf { it.isNotBlank() }))
                onDone()
            } catch (e: Exception) {
                // Stay on the screen so the user can retry.
            } finally {
                _busy.value = false
            }
        }
    }
}

/** «Мої інтереси» — edit prefs any time after onboarding (audit P0-4). */
@Composable
fun InterestsScreen(vm: InterestsViewModel, onDone: () -> Unit) {
    val prefs by vm.prefs.collectAsState()
    val busy by vm.busy.collectAsState()

    val current = prefs
    if (current == null) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { CircularProgressIndicator() }
        return
    }
    InterestsEditor(
        title = "Мої інтереси",
        initialCategories = current.categories,
        initialCity = current.city ?: "",
        busy = busy,
        skipLabel = null,
        onDone = { categories, city -> vm.save(categories, city) { onDone() } },
    )
}
