package com.underground.app

import com.underground.app.data.AuthResp
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Test

class AuthDtoTest {
    private val json = Json { ignoreUnknownKeys = true }

    @Test
    fun parsesAuthRespWithSnakeCaseUser() {
        val raw = """
            {"token":"abc.def","user":{"id":"1","email":"a@b.co","name":"Alice",
             "role":"USER","avatar_url":null,"bio":null,"created_at":"2026-06-12T00:00:00Z"}}
        """.trimIndent()
        val r = json.decodeFromString<AuthResp>(raw)
        assertEquals("abc.def", r.token)
        assertEquals("a@b.co", r.user.email)
        assertEquals("USER", r.user.role)
        assertEquals("Alice", r.user.name)
    }
}
