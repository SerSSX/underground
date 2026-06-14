package com.underground.app.util

import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

object Format {
    private val MONTHS = listOf("січ", "лют", "бер", "кві", "тра", "чер", "лип", "сер", "вер", "жов", "лис", "гру")

    /** ISO timestamp → "9 чер" (minSdk 24 lacks java.time without desugaring). */
    fun eventDate(iso: String?): String {
        if (iso == null || iso.length < 10) return ""
        val month = iso.substring(5, 7).toIntOrNull()?.let { MONTHS.getOrNull(it - 1) } ?: return iso.take(10)
        val day = iso.substring(8, 10).trimStart('0')
        return "$day $month"
    }

    /** ISO timestamp → "9 чер · 19:00"; date-only when time is midnight-less. */
    fun eventDateTime(iso: String?): String {
        val date = eventDate(iso)
        if (iso == null || iso.length < 16 || date.isEmpty()) return date
        return "$date · ${iso.substring(11, 16)}"
    }

    /** ISO instant → epoch millis (UTC); null when unparseable. */
    fun isoToMillis(iso: String?): Long? {
        if (iso == null || iso.length < 19) return null
        val fmt = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss", Locale.US)
        fmt.timeZone = TimeZone.getTimeZone("UTC")
        return runCatching { fmt.parse(iso.take(19))?.time }.getOrNull()
    }

    /** True when the ISO instant is before now (UTC). Null/short strings → false. */
    fun isPast(iso: String?): Boolean {
        if (iso == null || iso.length < 19) return false
        val fmt = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss", Locale.US)
        fmt.timeZone = TimeZone.getTimeZone("UTC")
        return iso.take(19) < fmt.format(Date())
    }

    /** Display symbols per ISO currency (TASK-017 M5); falls back to the code. */
    private val CURRENCY_SYMBOLS = mapOf(
        "UAH" to "₴", "USD" to "$", "EUR" to "€", "GBP" to "£", "PLN" to "zł", "CZK" to "Kč",
        "HUF" to "Ft", "CHF" to "CHF", "SEK" to "kr", "DKK" to "kr", "NOK" to "kr", "TRY" to "₺",
        "RON" to "lei", "BGN" to "лв", "CAD" to "C$", "MXN" to "$", "BRL" to "R$", "ARS" to "$",
        "JPY" to "¥", "CNY" to "¥", "KRW" to "₩", "SGD" to "S$", "THB" to "฿", "INR" to "₹",
        "AED" to "AED", "ILS" to "₪", "EGP" to "E£", "ZAR" to "R", "NGN" to "₦", "AUD" to "A$",
        "NZD" to "NZ$",
    )

    fun currencySymbol(currency: String): String = CURRENCY_SYMBOLS[currency] ?: currency

    /** "650.00" + "EUR" → "650 €"; keeps non-zero cents ("750.50 ₴"). */
    fun price(amount: String, currency: String): String {
        val trimmed = amount.removeSuffix(".00").ifEmpty { "0" }
        return "$trimmed ${currencySymbol(currency)}"
    }
}
