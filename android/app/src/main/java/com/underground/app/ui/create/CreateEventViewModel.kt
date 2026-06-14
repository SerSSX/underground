package com.underground.app.ui.create

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.underground.app.UndergroundApp
import com.underground.app.data.CreateEventReq
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.MultipartBody
import okhttp3.RequestBody.Companion.toRequestBody
import retrofit2.HttpException

class CreateEventViewModel : ViewModel() {
    private val api = UndergroundApp.container.eventApi

    private val _busy = MutableStateFlow(false)
    val busy: StateFlow<Boolean> = _busy.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    private val _done = MutableStateFlow(false)
    val done: StateFlow<Boolean> = _done.asStateFlow()

    private val _uploading = MutableStateFlow(false)
    val uploading: StateFlow<Boolean> = _uploading.asStateFlow()

    /** Uploads picked image bytes; returns the public URL via [onUrl]. */
    fun upload(bytes: ByteArray, mime: String, onUrl: (String) -> Unit) {
        viewModelScope.launch {
            _uploading.value = true
            _error.value = null
            try {
                val body = bytes.toRequestBody(mime.toMediaType())
                val part = MultipartBody.Part.createFormData("file", "image", body)
                onUrl(api.upload(part).url)
            } catch (e: Exception) {
                _error.value = "Не вдалося завантажити фото"
            } finally {
                _uploading.value = false
            }
        }
    }

    fun submit(req: CreateEventReq) {
        viewModelScope.launch {
            _busy.value = true
            _error.value = null
            try {
                api.create(req)
                _done.value = true
            } catch (e: HttpException) {
                _error.value = if (e.code() == 422) "Перевірте поля форми" else "Помилка ${e.code()}"
            } catch (e: Exception) {
                _error.value = e.message ?: "Помилка мережі"
            } finally {
                _busy.value = false
            }
        }
    }
}
