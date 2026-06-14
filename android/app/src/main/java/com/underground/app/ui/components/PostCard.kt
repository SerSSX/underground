package com.underground.app.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Comment
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.FavoriteBorder
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import com.underground.app.data.PostDto
import com.underground.app.ui.theme.Accent
import com.underground.app.ui.theme.Muted
import com.underground.app.util.Format
import com.underground.app.util.categoryColor

/** A wall post: author, optional review stars, text, attached event mini-card, social row. */
@Composable
fun PostCard(
    post: PostDto,
    isMine: Boolean,
    onToggleLike: () -> Unit,
    onOpenComments: () -> Unit,
    onOpenEvent: (String) -> Unit,
    onOpenAuthor: (String) -> Unit,
    onDelete: (() -> Unit)? = null,
) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
    ) {
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    modifier = Modifier
                        .size(36.dp)
                        .clip(CircleShape)
                        .background(Accent)
                        .clickable { onOpenAuthor(post.authorId) },
                    contentAlignment = Alignment.Center,
                ) {
                    Text(post.authorName.take(1).uppercase(), color = Color.White, fontWeight = FontWeight.Bold)
                }
                Spacer(Modifier.width(10.dp))
                Column(Modifier.weight(1f)) {
                    Text(post.authorName, fontWeight = FontWeight.SemiBold, maxLines = 1)
                    Text(
                        listOfNotNull(kindLabel(post.kind), Format.eventDate(post.createdAt), reasonLabel(post.reason))
                            .joinToString(" · "),
                        style = MaterialTheme.typography.labelSmall,
                        color = Muted,
                    )
                }
                post.rating?.let { rating ->
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        repeat(rating) {
                            Icon(
                                Icons.Filled.Star,
                                contentDescription = null,
                                tint = Accent,
                                modifier = Modifier.size(16.dp),
                            )
                        }
                    }
                }
                if (isMine && onDelete != null) {
                    IconButton(onClick = onDelete) {
                        Icon(Icons.Filled.Delete, contentDescription = "Видалити", tint = Muted)
                    }
                }
            }

            post.text?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }

            if (post.eventId != null && post.eventTitle != null) {
                EventMiniCard(post) { onOpenEvent(post.eventId) }
            }

            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onToggleLike) {
                    Icon(
                        if (post.likedByMe) Icons.Filled.Favorite else Icons.Filled.FavoriteBorder,
                        contentDescription = "Лайк",
                        tint = if (post.likedByMe) Accent else Muted,
                    )
                }
                Text("${post.likesCount}", color = Muted)
                Spacer(Modifier.width(16.dp))
                IconButton(onClick = onOpenComments) {
                    Icon(Icons.AutoMirrored.Filled.Comment, contentDescription = "Коментарі", tint = Muted)
                }
                Text("${post.commentsCount}", color = Muted)
            }
        }
    }
}

@Composable
private fun EventMiniCard(post: PostDto, onClick: () -> Unit) {
    val category = post.eventCategory.orEmpty()
    Surface(
        onClick = onClick,
        shape = RoundedCornerShape(12.dp),
        color = MaterialTheme.colorScheme.surfaceVariant,
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier
                    .size(width = 72.dp, height = 56.dp)
                    .background(categoryColor(category).copy(alpha = 0.85f)),
            ) {
                AsyncImage(
                    model = post.eventPoster,
                    contentDescription = null,
                    contentScale = ContentScale.Crop,
                    modifier = Modifier.matchParentSize(),
                )
            }
            Column(Modifier.padding(horizontal = 10.dp, vertical = 6.dp)) {
                Text(post.eventTitle.orEmpty(), fontWeight = FontWeight.SemiBold, maxLines = 1)
                Text(
                    listOfNotNull(category.ifEmpty { null }, Format.eventDate(post.eventStartsAt).ifEmpty { null })
                        .joinToString(" · "),
                    style = MaterialTheme.typography.labelSmall,
                    color = Muted,
                )
            }
        }
    }
}

private fun kindLabel(kind: String): String = when (kind) {
    "event_share" -> "анонс події"
    "review" -> "відгук"
    else -> "пост"
}

private fun reasonLabel(reason: String?): String? = when (reason) {
    "following" -> "ви підписані"
    "interest" -> "за інтересом"
    else -> null
}
