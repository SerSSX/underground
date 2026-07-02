package com.underground.app.ui.staff

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.AcceptStaffReq
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import retrofit2.HttpException

/** A door-controller invite parsed from `underground://staff/<token>?e=<eventId>&t=<title>`. */
data class StaffInvite(val token: String, val eventId: String?, val eventTitle: String?)

sealed interface AcceptState {
    data object Accepting : AcceptState
    data object Accepted : AcceptState
    data class Failed(val message: String) : AcceptState
}

class AcceptStaffViewModel : ViewModel() {
    private val api = UndergroundApp.container.eventApi

    private val _state = MutableStateFlow<AcceptState>(AcceptState.Accepting)
    val state: StateFlow<AcceptState> = _state.asStateFlow()

    fun accept(token: String) {
        viewModelScope.launch {
            _state.value = AcceptState.Accepting
            _state.value = try {
                api.acceptStaff(AcceptStaffReq(token))
                AcceptState.Accepted
            } catch (e: HttpException) {
                AcceptState.Failed(
                    when (e.code()) {
                        404 -> "Запрошення не знайдено або вже використане"
                        409 -> "Термін дії запрошення минув"
                        401 -> "Спочатку увійдіть у застосунок"
                        else -> "Помилка ${e.code()}"
                    },
                )
            } catch (e: Exception) {
                AcceptState.Failed(e.message ?: "Помилка мережі")
            }
        }
    }
}
