package com.underground.app.util

import android.net.Uri

/** Builds a Google Maps search link from a free-text address (no API key needed). */
object Maps {
    private fun query(address: String?, city: String?): String =
        listOfNotNull(address?.takeIf { it.isNotBlank() }, city?.takeIf { it.isNotBlank() }).joinToString(", ")

    fun hasLocation(address: String?, city: String?): Boolean = query(address, city).isNotBlank()

    fun url(address: String?, city: String?): String =
        "https://www.google.com/maps/search/?api=1&query=" + Uri.encode(query(address, city))
}
