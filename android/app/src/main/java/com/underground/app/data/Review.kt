package com.underground.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class ReviewDto(
    val id: String,
    @SerialName("event_id") val eventId: String,
    @SerialName("author_id") val authorId: String,
    @SerialName("author_name") val authorName: String,
    val rating: Int,
    val text: String? = null,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class EventReviewsDto(
    @SerialName("avg_rating") val avgRating: Double? = null,
    val count: Long = 0,
    val reviews: List<ReviewDto> = emptyList(),
)

@Serializable
data class PostReviewReq(
    @SerialName("event_id") val eventId: String,
    val rating: Int,
    val text: String? = null,
)
