package com.underground.app.ui.tickets

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.CheckoutResp
import com.underground.app.data.TicketDto
import com.underground.app.data.TransferReq
import com.underground.app.data.serverMessage
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import retrofit2.HttpException

class TicketsViewModel : ViewModel() {
    private val api = UndergroundApp.container.eventApi

    private val _tickets = MutableStateFlow<List<TicketDto>>(emptyList())
    val tickets: StateFlow<List<TicketDto>> = _tickets.asStateFlow()

    private val _loading = MutableStateFlow(true)
    val loading: StateFlow<Boolean> = _loading.asStateFlow()

    init {
        load()
    }

    private suspend fun fetch() {
        _tickets.value = try {
            api.myTickets()
        } catch (e: Exception) {
            emptyList()
        }
    }

    fun load() {
        viewModelScope.launch {
            _loading.value = true
            fetch()
            _loading.value = false
        }
    }

    fun find(id: String): TicketDto? = _tickets.value.firstOrNull { it.id == id }

    fun reserve(eventId: String, onSuccess: (TicketDto) -> Unit, onError: (String) -> Unit) {
        viewModelScope.launch {
            try {
                val ticket = api.reserve(eventId)
                fetch()
                onSuccess(ticket)
            } catch (e: HttpException) {
                onError(
                    when (e.code()) {
                        // Surface the server's own reason (e.g. the age-gate text) when present.
                        409 -> e.serverMessage() ?: "Квитки закінчились або подія платна"
                        401 -> "Спочатку увійдіть"
                        else -> "Помилка ${e.code()}"
                    },
                )
            } catch (e: Exception) {
                onError(e.message ?: "Помилка мережі")
            }
        }
    }

    fun checkout(eventId: String, onReady: (CheckoutResp) -> Unit, onError: (String) -> Unit) {
        viewModelScope.launch {
            try {
                onReady(api.checkout(eventId))
            } catch (e: HttpException) {
                onError(
                    when (e.code()) {
                        409 -> e.serverMessage() ?: "Безкоштовна подія або квитки закінчились"
                        401 -> "Спочатку увійдіть"
                        else -> "Помилка ${e.code()}"
                    },
                )
            } catch (e: Exception) {
                onError(e.message ?: "Помилка мережі")
            }
        }
    }

    fun payTest(orderId: String, onSuccess: (TicketDto) -> Unit, onError: (String) -> Unit) {
        viewModelScope.launch {
            try {
                val ticket = api.payTest(orderId)
                fetch()
                onSuccess(ticket)
            } catch (e: HttpException) {
                onError(
                    when (e.code()) {
                        409 -> "Оплата недоступна (не тест-режим або вже оплачено)"
                        404 -> "Замовлення не знайдено"
                        else -> "Помилка ${e.code()}"
                    },
                )
            } catch (e: Exception) {
                onError(e.message ?: "Помилка мережі")
            }
        }
    }

    fun returnTicket(ticketId: String, onDone: () -> Unit, onError: (String) -> Unit) {
        viewModelScope.launch {
            try {
                api.returnTicket(ticketId)
                fetch()
                onDone()
            } catch (e: HttpException) {
                onError(if (e.code() == 409) "Квиток уже використаний або повернений" else "Помилка ${e.code()}")
            } catch (e: Exception) {
                onError(e.message ?: "Помилка мережі")
            }
        }
    }

    fun dispute(orderId: String, onDone: () -> Unit, onError: (String) -> Unit) {
        viewModelScope.launch {
            try {
                api.dispute(orderId)
                fetch()
                onDone()
            } catch (e: HttpException) {
                onError(e.serverMessage() ?: "Не вдалося відкрити спір (помилка ${e.code()})")
            } catch (e: Exception) {
                onError(e.message ?: "Помилка мережі")
            }
        }
    }

    fun transferTicket(ticketId: String, email: String, onDone: () -> Unit, onError: (String) -> Unit) {
        viewModelScope.launch {
            try {
                api.transfer(ticketId, TransferReq(email.trim()))
                fetch()
                onDone()
            } catch (e: HttpException) {
                onError(
                    when (e.code()) {
                        422 -> "Невідомий отримувач або не можна подарувати собі"
                        404 -> "Квиток недоступний"
                        else -> "Помилка ${e.code()}"
                    },
                )
            } catch (e: Exception) {
                onError(e.message ?: "Помилка мережі")
            }
        }
    }
}
