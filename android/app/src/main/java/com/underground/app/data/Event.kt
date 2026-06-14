package com.underground.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** Mirrors the API `/api/events` item (snake_case JSON). */
@Serializable
data class EventDto(
    val id: String,
    @SerialName("organizer_id") val organizerId: String? = null,
    val title: String,
    @SerialName("poster_url") val posterUrl: String? = null,
    val category: String,
    val city: String? = null,
    @SerialName("venue_address") val venueAddress: String? = null,
    @SerialName("starts_at") val startsAt: String? = null,
    @SerialName("ends_at") val endsAt: String? = null,
    val price: String,
    val currency: String,
    @SerialName("tickets_left") val ticketsLeft: Int,
    val status: String,
    @SerialName("cost_type") val costType: String = "paid",
    val description: String? = null,
    @SerialName("video_url") val videoUrl: String? = null,
    @SerialName("gallery_urls") val galleryUrls: List<String> = emptyList(),
    val links: List<String> = emptyList(),
    @SerialName("sales_closed") val salesClosed: Boolean = false,
    @SerialName("age_rating") val ageRating: String? = null,
    val country: String? = null,
    val lat: Double? = null,
    val lng: Double? = null,
    // Discovery-only fields (present on /api/discover responses).
    val reason: String? = null,
    @SerialName("saved_by_me") val savedByMe: Boolean = false,
    @SerialName("saves_count") val savesCount: Long = 0,
    @SerialName("distance_km") val distanceKm: Double? = null,
)

val EventDto.isFree: Boolean
    get() = costType.equals("free", ignoreCase = true)
