package com.underground.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import okhttp3.MultipartBody
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.Multipart
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Part
import retrofit2.http.Path

@Serializable
data class CreateEventReq(
    val title: String,
    val description: String? = null,
    val category: String,
    val city: String? = null,
    @SerialName("venue_address") val venueAddress: String? = null,
    @SerialName("starts_at") val startsAt: String? = null,
    val price: String,
    val currency: String = "UAH",
    @SerialName("tickets_total") val ticketsTotal: Int,
    @SerialName("cost_type") val costType: String = "paid",
    @SerialName("poster_url") val posterUrl: String? = null,
    @SerialName("video_url") val videoUrl: String? = null,
    @SerialName("gallery_urls") val galleryUrls: List<String> = emptyList(),
    val links: List<String> = emptyList(),
    @SerialName("age_rating") val ageRating: String? = null,
)

/** Partial update — null fields stay untouched on the server. */
@Serializable
data class UpdateEventReq(
    val title: String? = null,
    val description: String? = null,
    val category: String? = null,
    val city: String? = null,
    @SerialName("venue_address") val venueAddress: String? = null,
    @SerialName("starts_at") val startsAt: String? = null,
    val price: String? = null,
    @SerialName("poster_url") val posterUrl: String? = null,
    @SerialName("video_url") val videoUrl: String? = null,
    @SerialName("gallery_urls") val galleryUrls: List<String>? = null,
    val links: List<String>? = null,
    val status: String? = null,
    @SerialName("sales_closed") val salesClosed: Boolean? = null,
    @SerialName("age_rating") val ageRating: String? = null,
)

@Serializable
data class UploadResp(val url: String)

@Serializable
data class CheckoutResp(
    @SerialName("order_id") val orderId: String,
    val mode: String,
    val amount: String,
    val currency: String,
    @SerialName("test_mode") val testMode: Boolean,
)

@Serializable
data class ValidateReq(@SerialName("qr_token") val qrToken: String)

@Serializable
data class TransferReq(val email: String)

@Serializable
data class ValidateResp(
    val valid: Boolean,
    val reason: String,
    @SerialName("event_title") val eventTitle: String,
)

interface EventApi {
    @GET("api/events")
    suspend fun getEvents(): List<EventDto>

    @GET("api/me/events")
    suspend fun myEvents(): List<EventDto>

    @GET("api/events/{id}")
    suspend fun getEvent(@Path("id") id: String): EventDto

    @GET("api/discover")
    suspend fun discover(): List<EventDto>

    @GET("api/me/saves")
    suspend fun mySaves(): List<EventDto>

    @POST("api/events/{id}/save")
    suspend fun saveEvent(@Path("id") id: String)

    @DELETE("api/events/{id}/save")
    suspend fun unsaveEvent(@Path("id") id: String)

    @POST("api/events")
    suspend fun create(@Body req: CreateEventReq): EventDto

    @PUT("api/events/{id}")
    suspend fun update(@Path("id") id: String, @Body req: UpdateEventReq): EventDto

    @DELETE("api/events/{id}")
    suspend fun cancel(@Path("id") id: String)

    @Multipart
    @POST("api/uploads")
    suspend fun upload(@Part file: MultipartBody.Part): UploadResp

    @POST("api/events/{id}/reserve")
    suspend fun reserve(@Path("id") id: String): TicketDto

    @POST("api/events/{id}/checkout")
    suspend fun checkout(@Path("id") id: String): CheckoutResp

    @POST("api/orders/{id}/pay-test")
    suspend fun payTest(@Path("id") id: String): TicketDto

    @GET("api/events/{id}/reviews")
    suspend fun reviews(@Path("id") id: String): EventReviewsDto

    @GET("api/events/{id}/social")
    suspend fun social(@Path("id") id: String): EventSocialDto

    @GET("api/events/{id}/similar")
    suspend fun similar(@Path("id") id: String): List<EventDto>

    @POST("api/reviews")
    suspend fun postReview(@Body req: PostReviewReq): ReviewDto

    @GET("api/me/tickets")
    suspend fun myTickets(): List<TicketDto>

    @POST("api/tickets/validate")
    suspend fun validate(@Body req: ValidateReq): ValidateResp

    @POST("api/tickets/{id}/return")
    suspend fun returnTicket(@Path("id") id: String): TicketDto

    @POST("api/tickets/{id}/transfer")
    suspend fun transfer(@Path("id") id: String, @Body req: TransferReq): TicketDto

    // Buyer opens a dispute on the paid order behind a ticket (freezes escrow).
    @POST("api/orders/{id}/dispute")
    suspend fun dispute(@Path("id") id: String)
}
