package com.underground.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class TicketDto(
    val id: String,
    @SerialName("event_id") val eventId: String,
    @SerialName("qr_token") val qrToken: String,
    val status: String,
    val price: String,
    @SerialName("event_title") val eventTitle: String,
    @SerialName("event_starts_at") val eventStartsAt: String? = null,
    @SerialName("event_city") val eventCity: String? = null,
    @SerialName("event_venue_address") val eventVenueAddress: String? = null,
    @SerialName("event_poster_url") val eventPosterUrl: String? = null,
    /** Paid order behind this ticket (null for free) — enables "dispute". */
    @SerialName("order_id") val orderId: String? = null,
    /** held|clearing → dispute possible; frozen → already disputed; else not disputable. */
    @SerialName("escrow_status") val escrowStatus: String? = null,
)
