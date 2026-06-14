package com.underground.app.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Bookmark
import androidx.compose.material.icons.filled.BookmarkBorder
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
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import kotlin.math.roundToInt
import com.underground.app.data.EventDto
import com.underground.app.ui.theme.Accent
import com.underground.app.ui.theme.Muted
import com.underground.app.util.Format
import com.underground.app.util.categoryColor

@Composable
fun EventCard(
    event: EventDto,
    saved: Boolean = false,
    reason: String? = null,
    onToggleSave: (() -> Unit)? = null,
    onClick: () -> Unit,
) {
    Card(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth(),
        shape = RoundedCornerShape(18.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
    ) {
        Box(
            Modifier
                .fillMaxWidth()
                .height(180.dp)
                .background(
                    Brush.verticalGradient(
                        listOf(categoryColor(event.category).copy(alpha = 0.9f), Color(0xFF0B0B0D)),
                    ),
                ),
        ) {
            AsyncImage(
                model = event.posterUrl,
                contentDescription = event.title,
                contentScale = ContentScale.Crop,
                modifier = Modifier.matchParentSize(),
            )
            Pill(event.category, categoryColor(event.category), Modifier.align(Alignment.TopStart).padding(12.dp))
            if (onToggleSave != null) {
                Surface(
                    color = Color(0xCC000000),
                    shape = CircleShape,
                    modifier = Modifier.align(Alignment.TopEnd).padding(8.dp),
                ) {
                    IconButton(onClick = onToggleSave, modifier = Modifier.size(36.dp)) {
                        Icon(
                            if (saved) Icons.Filled.Bookmark else Icons.Filled.BookmarkBorder,
                            contentDescription = if (saved) "Збережено" else "Зберегти",
                            tint = if (saved) Accent else Color.White,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }
            } else {
                Pill("лишилось ${event.ticketsLeft}", Color(0xCC000000), Modifier.align(Alignment.TopEnd).padding(12.dp))
            }
            Text(
                text = Format.eventDateTime(event.startsAt),
                style = MaterialTheme.typography.labelLarge,
                color = Color.White,
                modifier = Modifier.align(Alignment.BottomStart).padding(12.dp),
            )
        }
        Column(Modifier.padding(14.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            reasonLabel(reason)?.let {
                Text(it, style = MaterialTheme.typography.labelSmall, color = Accent, fontWeight = FontWeight.SemiBold)
            }
            Text(
                event.title,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                maxLines = 2,
            )
            val distance = event.distanceKm?.let { if (it < 1.0) "поряд" else "~${it.roundToInt()} км" }
            Text(
                listOfNotNull(event.city, distance ?: event.venueAddress).joinToString(" · "),
                style = MaterialTheme.typography.bodySmall,
                color = Muted,
                maxLines = 1,
            )
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text(
                    Format.price(event.price, event.currency),
                    style = MaterialTheme.typography.titleMedium,
                    color = Accent,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(top = 2.dp),
                )
                if (event.savesCount > 0) {
                    Text(
                        "${event.savesCount} цікавляться",
                        style = MaterialTheme.typography.labelSmall,
                        color = Muted,
                        modifier = Modifier.padding(top = 6.dp),
                    )
                }
            }
        }
    }
}

private fun reasonLabel(reason: String?): String? = when (reason) {
    "followed" -> "★ ВИ ПІДПИСАНІ"
    "interest" -> "ЗА ІНТЕРЕСОМ"
    "city" -> "ПОРЯД З ВАМИ"
    "trending" -> "🔥 ПОПУЛЯРНЕ"
    else -> null
}

@Composable
private fun Pill(text: String, color: Color, modifier: Modifier = Modifier) {
    Surface(color = color, shape = RoundedCornerShape(50), modifier = modifier) {
        Text(
            text,
            style = MaterialTheme.typography.labelMedium,
            color = Color.White,
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
        )
    }
}
