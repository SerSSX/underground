package com.underground.app.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.DeleteMeReq
import com.underground.app.data.KycReq
import com.underground.app.data.PatchMeReq
import com.underground.app.data.UserDto
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.MultipartBody
import okhttp3.RequestBody.Companion.toRequestBody

class SettingsViewModel : ViewModel() {
    private val container = UndergroundApp.container

    private val _busy = MutableStateFlow(false)
    val busy: StateFlow<Boolean> = _busy.asStateFlow()

    private val _uploading = MutableStateFlow(false)
    val uploading: StateFlow<Boolean> = _uploading.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    private val _message = MutableStateFlow<String?>(null)
    val message: StateFlow<String?> = _message.asStateFlow()

    private val _kyc = MutableStateFlow<String?>(null)
    val kyc: StateFlow<String?> = _kyc.asStateFlow()

    fun loadKyc() {
        viewModelScope.launch {
            try {
                _kyc.value = container.authApi.getKyc().kycStatus
            } catch (_: Exception) {
                /* organizer KYC is optional context */
            }
        }
    }

    /** Save date of birth (YYYY-MM-DD) for age-gating (P1 #77). */
    fun saveDob(dob: String) {
        if (!isValidDob(dob)) {
            _error.value = "Невірна дата. Формат: РРРР-ММ-ДД"
            return
        }
        viewModelScope.launch {
            _busy.value = true
            _error.value = null
            _message.value = null
            try {
                container.authApi.patchMe(PatchMeReq(dob = dob))
                _message.value = "Дату народження збережено"
            } catch (e: Exception) {
                _error.value = "Не вдалося зберегти дату народження"
            } finally {
                _busy.value = false
            }
        }
    }

    /** Submit organizer KYC bank details for review (P1 #78). */
    fun submitKyc(name: String, iban: String, swift: String) {
        viewModelScope.launch {
            _busy.value = true
            _error.value = null
            _message.value = null
            try {
                container.authApi.submitKyc(KycReq(name.trim(), iban.trim(), swift.trim()))
                _message.value = "Реквізити надіслано на перевірку"
                loadKyc()
            } catch (e: Exception) {
                _error.value = "Перевірте реквізити (IBAN/SWIFT)"
            } finally {
                _busy.value = false
            }
        }
    }

    /** GDPR export (P1 #73): hands the raw JSON to [onJson] for sharing. */
    fun exportData(onJson: (String) -> Unit) {
        viewModelScope.launch {
            _error.value = null
            try {
                onJson(container.authApi.exportMe().toString())
            } catch (e: Exception) {
                _error.value = "Не вдалося експортувати дані"
            }
        }
    }

    /** GDPR erasure (P1 #73): re-auth with [password]; on success calls [onDone]. */
    fun deleteAccount(password: String, onDone: () -> Unit) {
        viewModelScope.launch {
            _busy.value = true
            _error.value = null
            try {
                container.authApi.deleteMe(DeleteMeReq(password))
                onDone()
            } catch (e: Exception) {
                _error.value = "Невірний пароль або помилка видалення"
            } finally {
                _busy.value = false
            }
        }
    }

    fun uploadAvatar(bytes: ByteArray, mime: String, onUrl: (String) -> Unit) {
        viewModelScope.launch {
            _uploading.value = true
            _error.value = null
            try {
                val body = bytes.toRequestBody(mime.toMediaType())
                val part = MultipartBody.Part.createFormData("file", "avatar", body)
                onUrl(container.eventApi.upload(part).url)
            } catch (e: Exception) {
                _error.value = "Не вдалося завантажити аватар"
            } finally {
                _uploading.value = false
            }
        }
    }

    /** An empty bio is sent as "" so the server clears it; a blank name is rejected upstream. */
    fun save(name: String, bio: String, avatarUrl: String?, isOrganizer: Boolean, onSaved: (UserDto) -> Unit) {
        viewModelScope.launch {
            _busy.value = true
            _error.value = null
            try {
                val updated = container.authApi.patchMe(
                    PatchMeReq(
                        name = name.trim(),
                        bio = bio.trim(),
                        avatarUrl = avatarUrl,
                        isOrganizer = isOrganizer,
                    ),
                )
                onSaved(updated)
            } catch (e: Exception) {
                _error.value = "Не вдалося зберегти зміни"
            } finally {
                _busy.value = false
            }
        }
    }

    companion object {
        /**
         * True only for a real, non-future calendar date in strict YYYY-MM-DD form.
         * Uses lenient=false so "2008-13-40" and "2008-02-30" are rejected, and blocks
         * future dates / years before 1900. The Save DoB button gates on this.
         */
        fun isValidDob(dob: String): Boolean {
            if (!Regex("""^\d{4}-\d{2}-\d{2}$""").matches(dob)) return false
            val fmt = java.text.SimpleDateFormat("yyyy-MM-dd", java.util.Locale.US).apply { isLenient = false }
            val parsed = try {
                fmt.parse(dob)
            } catch (_: Exception) {
                null
            } ?: return false
            if (parsed.after(java.util.Date())) return false
            val year = java.util.Calendar.getInstance().apply { time = parsed }.get(java.util.Calendar.YEAR)
            return year >= 1900
        }
    }
}
