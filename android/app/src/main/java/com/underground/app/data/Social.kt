package com.underground.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class PostDto(
    val id: String,
    @SerialName("author_id") val authorId: String,
    @SerialName("author_name") val authorName: String,
    @SerialName("author_avatar") val authorAvatar: String? = null,
    val kind: String, // text | event_share | review
    val text: String? = null,
    val rating: Int? = null,
    @SerialName("event_id") val eventId: String? = null,
    @SerialName("event_title") val eventTitle: String? = null,
    @SerialName("event_poster") val eventPoster: String? = null,
    @SerialName("event_category") val eventCategory: String? = null,
    @SerialName("event_starts_at") val eventStartsAt: String? = null,
    @SerialName("likes_count") val likesCount: Long = 0,
    @SerialName("comments_count") val commentsCount: Long = 0,
    @SerialName("liked_by_me") val likedByMe: Boolean = false,
    @SerialName("created_at") val createdAt: String,
    /** Feed only: `following` | `interest`. */
    val reason: String? = null,
)

@Serializable
data class CreatePostReq(
    val text: String? = null,
    @SerialName("event_id") val eventId: String? = null,
)

@Serializable
data class CommentDto(
    val id: String,
    @SerialName("post_id") val postId: String,
    @SerialName("author_id") val authorId: String,
    @SerialName("author_name") val authorName: String,
    val text: String,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class CreateCommentReq(val text: String)

@Serializable
data class MessageDto(
    val id: String,
    @SerialName("sender_id") val senderId: String,
    @SerialName("recipient_id") val recipientId: String,
    val text: String,
    val read: Boolean = false,
    @SerialName("created_at") val createdAt: String,
)

@Serializable
data class SendMessageReq(
    @SerialName("recipient_id") val recipientId: String,
    val text: String,
)

@Serializable
data class UserBriefDto(
    val id: String,
    val name: String,
    @SerialName("avatar_url") val avatarUrl: String? = null,
)

@Serializable
data class EventSocialDto(
    @SerialName("interested_count") val interestedCount: Long = 0,
    @SerialName("going_count") val goingCount: Long = 0,
    @SerialName("friends_interested") val friendsInterested: List<UserBriefDto> = emptyList(),
    @SerialName("friends_going") val friendsGoing: List<UserBriefDto> = emptyList(),
)

@Serializable
data class ConversationDto(
    @SerialName("peer_id") val peerId: String,
    @SerialName("peer_name") val peerName: String,
    @SerialName("peer_avatar") val peerAvatar: String? = null,
    @SerialName("last_text") val lastText: String,
    @SerialName("last_at") val lastAt: String,
    @SerialName("last_from_me") val lastFromMe: Boolean = false,
    val unread: Long = 0,
)
