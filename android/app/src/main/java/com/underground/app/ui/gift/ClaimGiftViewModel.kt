package com.underground.app.ui.gift

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.ClaimReq
import com.underground.app.data.GiftPreviewDto
import com.underground.app.data.RequestCodeReq
import com.underground.app.data.VerifyCodeReq
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import retrofit2.HttpException

/** Gift claim flow (TASK-114, WF-GiftClaim): preview → (passwordless auth) → claim. */
sealed interface ClaimState {
    data object Loading : ClaimState
    data object Gone : ClaimState
    data class Ready(val preview: GiftPreviewDto) : ClaimState
    data object Done : ClaimState
}

class ClaimGiftViewModel : ViewModel() {
    private val container = UndergroundApp.container
    private var token: String = ""

    private val _state = MutableStateFlow<ClaimState>(ClaimState.Loading)
    val state: StateFlow<ClaimState> = _state.asStateFlow()
    private val _busy = MutableStateFlow(false)
    val busy: StateFlow<Boolean> = _busy.asStateFlow()
    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()
    private val _codeSent = MutableStateFlow(false)
    val codeSent: StateFlow<Boolean> = _codeSent.asStateFlow()

    /** Whether we already have a session — if so, claiming is a single tap. */
    val loggedIn: Boolean get() = container.tokenStore.token != null

    /** Load the public gift preview (browse-before-auth). Idempotent per token. */
    fun load(giftToken: String) {
        if (token == giftToken && _state.value !is ClaimState.Loading) return
        token = giftToken
        viewModelScope.launch {
            _state.value = try {
                ClaimState.Ready(container.eventApi.giftPreview(giftToken))
            } catch (e: Exception) {
                ClaimState.Gone
            }
        }
    }

    fun sendCode(email: String) = run {
        container.authApi.requestEmailCode(RequestCodeReq(email.trim()))
        _codeSent.value = true
    }

    fun verifyAndClaim(email: String, code: String) = run {
        val resp = container.authApi.verifyEmailCode(VerifyCodeReq(email.trim(), code.trim()))
        container.tokenStore.save(resp.token)
        container.eventApi.claim(ClaimReq(token))
        _state.value = ClaimState.Done
    }

    fun claim() = run {
        container.eventApi.claim(ClaimReq(token))
        _state.value = ClaimState.Done
    }

    private fun run(block: suspend () -> Unit) {
        viewModelScope.launch {
            _busy.value = true
            _error.value = null
            try {
                block()
            } catch (e: HttpException) {
                _error.value = when (e.code()) {
                    401 -> "Невірний код"
                    404, 409 -> "Подарунок недоступний або протермінований"
                    429 -> "Забагато спроб, спробуйте пізніше"
                    else -> "Помилка ${e.code()}"
                }
            } catch (e: Exception) {
                _error.value = e.message ?: "Помилка мережі"
            } finally {
                _busy.value = false
            }
        }
    }
}
