package com.underground.app

import com.underground.app.data.EventDto
import com.underground.app.ui.afisha.AfishaUiState
import com.underground.app.ui.afisha.filterByCategory
import com.underground.app.ui.afisha.reduce
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class EventDtoTest {
    private val json = Json { ignoreUnknownKeys = true }

    /** Minimal published event with only the fields these tests assert on. */
    private fun ev(id: String, title: String, category: String) = EventDto(
        id = id, title = title, category = category,
        price = "0", currency = "UAH", ticketsLeft = 0, status = "published",
    )

    @Test
    fun parsesSnakeCaseAndIgnoresUnknownFields() {
        val raw = """
            [{"id":"1","title":"LOUNA","poster_url":"http://x/p.png","category":"Rock",
              "city":"Київ","venue_address":"Atlas","starts_at":"2026-07-02T17:00:00Z",
              "ends_at":"2026-07-02T20:00:00Z","price":"650.00","currency":"UAH",
              "tickets_left":173,"status":"published","unknown_field":42}]
        """.trimIndent()

        val list = json.decodeFromString<List<EventDto>>(raw)

        assertEquals(1, list.size)
        val e = list[0]
        assertEquals("LOUNA", e.title)
        assertEquals("http://x/p.png", e.posterUrl)
        assertEquals("Atlas", e.venueAddress)
        assertEquals(173, e.ticketsLeft)
    }

    @Test
    fun reduceEmptyListYieldsEmptyState() {
        assertTrue(reduce(emptyList()) is AfishaUiState.Empty)
    }

    @Test
    fun reduceNonEmptyYieldsSuccess() {
        val e = ev("1", "T", "Rock")
        assertTrue(reduce(listOf(e)) is AfishaUiState.Success)
    }

    @Test
    fun filterByCategoryNullReturnsAll() {
        val rock = ev("1", "R", "Rock")
        val pop = ev("2", "P", "Pop")
        assertEquals(2, filterByCategory(listOf(rock, pop), null).size)
    }

    @Test
    fun filterByCategoryKeepsOnlyMatching() {
        val rock = ev("1", "R", "Rock")
        val pop = ev("2", "P", "Pop")
        val filtered = filterByCategory(listOf(rock, pop), "Rock")
        assertEquals(1, filtered.size)
        assertEquals("Rock", filtered[0].category)
    }
}
