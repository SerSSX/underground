package com.underground.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Path

@Serializable
data class PrefsDto(
    val categories: List<String> = emptyList(),
    val city: String? = null,
    val onboarded: Boolean = false,
)

@Serializable
data class PutPrefsReq(
    val categories: List<String>,
    val city: String? = null,
)

@Serializable
data class PublicProfileDto(
    val id: String,
    val name: String,
    val role: String,
    @SerialName("avatar_url") val avatarUrl: String? = null,
    val bio: String? = null,
    @SerialName("followers_count") val followersCount: Long = 0,
    @SerialName("following_count") val followingCount: Long = 0,
    @SerialName("events_count") val eventsCount: Long = 0,
    @SerialName("rating_avg") val ratingAvg: Double? = null,
    @SerialName("rating_count") val ratingCount: Long = 0,
    @SerialName("is_following") val isFollowing: Boolean = false,
    @SerialName("is_organizer") val isOrganizer: Boolean = false,
)

@Serializable
data class UnreadDto(
    val notifications: Long = 0,
    val messages: Long = 0,
)

@Serializable
data class NotificationDto(
    val id: String,
    val kind: String,
    val title: String,
    val body: String? = null,
    @SerialName("event_id") val eventId: String? = null,
    @SerialName("actor_id") val actorId: String? = null,
    val read: Boolean = false,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class RegisterDeviceReq(
    val token: String,
    val platform: String = "android",
)

@Serializable
data class CityDto(
    val name: String,
    val country: String,
    val lat: Double,
    val lng: Double,
)

interface UserApi {
    @GET("api/cities")
    suspend fun cities(@retrofit2.http.Query("q") q: String): List<CityDto>

    @GET("api/me/prefs")
    suspend fun getPrefs(): PrefsDto

    @PUT("api/me/prefs")
    suspend fun putPrefs(@Body req: PutPrefsReq): PrefsDto

    @GET("api/users/{id}")
    suspend fun profile(@Path("id") id: String): PublicProfileDto

    @GET("api/users/{id}/events")
    suspend fun userEvents(@Path("id") id: String): List<EventDto>

    @POST("api/users/{id}/follow")
    suspend fun follow(@Path("id") id: String)

    @DELETE("api/users/{id}/follow")
    suspend fun unfollow(@Path("id") id: String)

    @GET("api/me/unread")
    suspend fun unread(): UnreadDto

    @GET("api/me/notifications")
    suspend fun notifications(): List<NotificationDto>

    @POST("api/me/notifications/read")
    suspend fun markNotificationsRead()

    @POST("api/me/devices")
    suspend fun registerDevice(@Body req: RegisterDeviceReq)

    @GET("api/users/{id}/posts")
    suspend fun userPosts(@Path("id") id: String): List<PostDto>

    @GET("api/feed")
    suspend fun feed(): List<PostDto>

    @POST("api/posts")
    suspend fun createPost(@Body req: CreatePostReq): PostDto

    @DELETE("api/posts/{id}")
    suspend fun deletePost(@Path("id") id: String)

    @POST("api/posts/{id}/like")
    suspend fun likePost(@Path("id") id: String)

    @DELETE("api/posts/{id}/like")
    suspend fun unlikePost(@Path("id") id: String)

    @GET("api/posts/{id}/comments")
    suspend fun comments(@Path("id") id: String): List<CommentDto>

    @POST("api/posts/{id}/comments")
    suspend fun addComment(@Path("id") id: String, @Body req: CreateCommentReq): CommentDto

    @POST("api/messages")
    suspend fun sendMessage(@Body req: SendMessageReq): MessageDto

    @GET("api/messages/{userId}")
    suspend fun thread(@Path("userId") userId: String): List<MessageDto>

    @GET("api/me/conversations")
    suspend fun conversations(): List<ConversationDto>
}
