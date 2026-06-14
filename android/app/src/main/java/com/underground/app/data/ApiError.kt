package com.underground.app.data

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import retrofit2.HttpException

/**
 * The API serializes every error as `{"error": "<safe message>"}`. This pulls that message out
 * of an [HttpException] so the UI can surface the server's own wording (e.g. the age-gate
 * Conflict text) instead of a generic "Помилка 409". Returns null if the body is missing or
 * unparseable, so callers can fall back to a status-based default.
 */
fun HttpException.serverMessage(): String? = try {
    val raw = response()?.errorBody()?.string()
    if (raw.isNullOrBlank()) {
        null
    } else {
        Json { ignoreUnknownKeys = true }
            .parseToJsonElement(raw)
            .jsonObject["error"]
            ?.jsonPrimitive
            ?.content
            ?.takeIf { it.isNotBlank() }
    }
} catch (_: Exception) {
    null
}
