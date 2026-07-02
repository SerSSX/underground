package com.underground.app

import com.underground.app.util.Format
import org.junit.Assert.assertEquals
import org.junit.Test

class FormatTest {
    @Test
    fun eventDate_formatsDayAndUkrainianMonth() {
        assertEquals("2 лип", Format.eventDate("2026-07-02T17:00:00Z"))
    }

    @Test
    fun eventDate_nullIsEmpty() {
        assertEquals("", Format.eventDate(null))
    }

    @Test
    fun price_trimsZeroCentsAndUsesCurrencySymbol() {
        assertEquals("650 ₴", Format.price("650.00", "UAH"))
    }

    @Test
    fun price_keepsNonZeroCents() {
        assertEquals("750.50 ₴", Format.price("750.50", "UAH"))
    }
}
