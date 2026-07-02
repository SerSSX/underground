package com.underground.app.ui.auth

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.AuthResp
import com.underground.app.data.ForgotPasswordReq
import com.underground.app.data.LoginReq
import com.underground.app.data.PutPrefsReq
import com.underground.app.data.RegisterReq
import com.underground.app.data.UserDto
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import retrofit2.HttpException

sealed interface AuthState {
    data object Loading : AuthState
    data object LoggedOut : AuthState
    data class LoggedIn(val user: UserDto, val needsOnboarding: Boolean = false) : AuthState
}

class AuthViewModel : ViewModel() {
    private val container = UndergroundApp.container

    private val _state = MutableStateFlow<AuthState>(AuthState.Loading)
    val state: StateFlow<AuthState> = _state.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    private val _busy = MutableStateFlow(false)
    val busy: StateFlow<Boolean> = _busy.asStateFlow()

    init {
        bootstrap()
    }

    /** Re-evaluate the session (e.g. after a deep-link gift claim signed the user in). */
    fun refresh() = bootstrap()

    private fun bootstrap() {
        viewModelScope.launch {
            if (container.tokenStore.token == null) {
                _state.value = AuthState.LoggedOut
                return@launch
            }
            _state.value = try {
                val user = container.authApi.me()
                AuthState.LoggedIn(user, needsOnboarding = !isOnboarded())
            } catch (e: Exception) {
                container.tokenStore.clear()
                AuthState.LoggedOut
            }
        }
    }

    fun login(email: String, password: String) =
        submit { container.authApi.login(LoginReq(email.trim(), password)) }

    fun register(email: String, password: String, name: String) =
        submit { container.authApi.register(RegisterReq(email.trim(), password, name.trim())) }

    /**
     * Request a password-reset link. Always reports success (the API is silent for unknown
     * emails to avoid account enumeration); the reset itself completes via the link on the web.
     */
    fun forgotPassword(email: String, onResult: (String) -> Unit) {
        viewModelScope.launch {
            try {
                container.authApi.forgotPassword(ForgotPasswordReq(email.trim()))
            } catch (_: Exception) {
                // Swallow: we still show the generic confirmation regardless.
            }
            onResult("Якщо для цієї адреси є акаунт, ми надіслали посилання на пошту.")
        }
    }

    private fun submit(call: suspend () -> AuthResp) {
        viewModelScope.launch {
            _busy.value = true
            _error.value = null
            try {
                val resp = call()
                container.tokenStore.save(resp.token)
                _state.value = AuthState.LoggedIn(resp.user, needsOnboarding = !isOnboarded())
            } catch (e: HttpException) {
                _error.value = when (e.code()) {
                    401 -> "Невірний email або пароль"
                    409 -> "Email уже зареєстровано"
                    422 -> "Перевірте дані (email і пароль ≥ 8 символів)"
                    else -> "Помилка ${e.code()}"
                }
            } catch (e: Exception) {
                _error.value = e.message ?: "Помилка мережі"
            } finally {
                _busy.value = false
            }
        }
    }

    /** Onboarding gate is best-effort: a failed prefs call must not block login. */
    private suspend fun isOnboarded(): Boolean =
        try {
            container.userApi.getPrefs().onboarded
        } catch (e: Exception) {
            true
        }

    /** Saves onboarding prefs; an empty selection still marks the user onboarded. */
    fun completeOnboarding(categories: List<String>, city: String?) {
        val current = _state.value as? AuthState.LoggedIn ?: return
        viewModelScope.launch {
            _busy.value = true
            try {
                container.userApi.putPrefs(PutPrefsReq(categories, city?.takeIf { it.isNotBlank() }))
            } catch (e: Exception) {
                // Non-fatal: the user proceeds; prefs can be saved again later.
            } finally {
                _busy.value = false
                _state.value = current.copy(needsOnboarding = false)
            }
        }
    }

    /** Refresh the in-memory user after a profile edit so the UI updates instantly. */
    fun updateUser(user: UserDto) {
        val current = _state.value as? AuthState.LoggedIn ?: return
        _state.value = current.copy(user = user)
    }

    fun logout() {
        container.tokenStore.clear()
        _state.value = AuthState.LoggedOut
    }
}
