package com.underground.app.ui.detail

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Bookmark
import androidx.compose.material.icons.filled.BookmarkBorder
import androidx.compose.material.icons.filled.LocationOn
import androidx.compose.material.icons.filled.Event
import androidx.compose.material.icons.filled.MailOutline
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.PlayCircle
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import kotlinx.coroutines.launch
import com.underground.app.data.EventDto
import com.underground.app.data.isFree
import com.underground.app.ui.theme.Accent
import com.underground.app.ui.theme.Muted
import com.underground.app.util.Format
import com.underground.app.util.Maps
import com.underground.app.util.categoryColor

@Composable
fun EventDetailScreen(
    event: EventDto?,
    reviewsVm: ReviewsViewModel,
    social: com.underground.app.data.EventSocialDto? = null,
    similar: List<EventDto> = emptyList(),
    saved: Boolean = false,
    onToggleSave: (() -> Unit)? = null,
    onOpenEvent: (String) -> Unit = {},
    onReserve: (String) -> Unit,
    onBuy: (String) -> Unit,
    onOpenOrganizer: (String) -> Unit,
    onMessageOrganizer: (String) -> Unit,
    onBack: () -> Unit,
) {
    val uriHandler = LocalUriHandler.current
    val context = androidx.compose.ui.platform.LocalContext.current
    if (event == null) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            Text("Подію не знайдено")
        }
        return
    }

    val past = Format.isPast(event.startsAt)
    val scope = rememberCoroutineScope()
    var showReport by remember { mutableStateOf(false) }
    var reportReason by remember { mutableStateOf("") }
    fun share() {
        val url = "${com.underground.app.BuildConfig.WEB_BASE_URL}/events/${event.id}"
        val intent = android.content.Intent(android.content.Intent.ACTION_SEND).apply {
            type = "text/plain"
            putExtra(android.content.Intent.EXTRA_TEXT, "${event.title} — $url")
        }
        context.startActivity(android.content.Intent.createChooser(intent, "Поділитися подією"))
    }
    fun addToCalendar() {
        val intent = android.content.Intent(android.content.Intent.ACTION_INSERT).apply {
            data = android.provider.CalendarContract.Events.CONTENT_URI
            putExtra(android.provider.CalendarContract.Events.TITLE, event.title)
            putExtra(
                android.provider.CalendarContract.Events.EVENT_LOCATION,
                listOfNotNull(event.venueAddress, event.city).joinToString(", "),
            )
            event.description?.let { putExtra(android.provider.CalendarContract.Events.DESCRIPTION, it) }
            Format.isoToMillis(event.startsAt)?.let {
                putExtra(android.provider.CalendarContract.EXTRA_EVENT_BEGIN_TIME, it)
            }
        }
        runCatching { context.startActivity(intent) }.onFailure {
            android.widget.Toast.makeText(context, "Немає застосунку календаря", android.widget.Toast.LENGTH_SHORT).show()
        }
    }
    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
        Box(
            Modifier
                .fillMaxWidth()
                .height(280.dp)
                .background(
                    Brush.verticalGradient(
                        listOf(categoryColor(event.category).copy(alpha = 0.95f), Color(0xFF0B0B0D)),
                    ),
                ),
        ) {
            AsyncImage(
                model = event.posterUrl,
                contentDescription = event.title,
                contentScale = ContentScale.Crop,
                modifier = Modifier.matchParentSize(),
            )
            IconButton(onClick = onBack, modifier = Modifier.align(Alignment.TopStart).padding(8.dp)) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад", tint = Color.White)
            }
            Row(
                modifier = Modifier.align(Alignment.TopEnd).padding(8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                if (onToggleSave != null) {
                    Surface(color = Color(0xCC000000), shape = CircleShape) {
                        IconButton(onClick = onToggleSave, modifier = Modifier.size(40.dp)) {
                            Icon(
                                if (saved) Icons.Filled.Bookmark else Icons.Filled.BookmarkBorder,
                                contentDescription = if (saved) "Збережено" else "Зберегти",
                                tint = if (saved) Accent else Color.White,
                            )
                        }
                    }
                }
                Surface(color = categoryColor(event.category), shape = RoundedCornerShape(50)) {
                    Text(
                        event.category,
                        style = MaterialTheme.typography.labelMedium,
                        color = Color.White,
                        modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
                    )
                }
                event.ageRating?.let { ar ->
                    Surface(color = Color(0xCC000000), shape = RoundedCornerShape(50)) {
                        Text(
                            "$ar+",
                            style = MaterialTheme.typography.labelMedium,
                            color = Color.White,
                            modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
                        )
                    }
                }
            }
        }

        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(event.title, style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
            Text(
                listOfNotNull(event.city, Format.eventDateTime(event.startsAt).ifEmpty { null }).joinToString(" · "),
                style = MaterialTheme.typography.bodyMedium,
                color = Muted,
            )
            if (Maps.hasLocation(event.venueAddress, event.city)) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { uriHandler.openUri(Maps.url(event.venueAddress, event.city)) },
                ) {
                    Icon(Icons.Filled.LocationOn, contentDescription = null, tint = Accent)
                    Column(Modifier.padding(start = 6.dp)) {
                        Text(
                            event.venueAddress ?: event.city.orEmpty(),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        Text(
                            "Відкрити в Google Maps",
                            style = MaterialTheme.typography.labelSmall,
                            color = Accent,
                        )
                    }
                }
            }
            event.description?.let { Text(it, style = MaterialTheme.typography.bodyMedium) }

            social?.let { s ->
                if (s.interestedCount > 0 || s.goingCount > 0) {
                    Text(
                        "🔥 ${s.interestedCount} цікавляться · ${s.goingCount} піде",
                        style = MaterialTheme.typography.bodyMedium,
                        color = Accent,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
                val friends = (s.friendsGoing + s.friendsInterested).map { it.name }.distinct()
                if (friends.isNotEmpty()) {
                    Text(
                        "Серед них друзі: " + friends.take(3).joinToString(", ") +
                            if (friends.size > 3) " та ще ${friends.size - 3}" else "",
                        style = MaterialTheme.typography.bodySmall,
                        color = Muted,
                    )
                }
            }

            if (event.galleryUrls.isNotEmpty()) {
                LazyRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(event.galleryUrls) { url ->
                        AsyncImage(
                            model = url,
                            contentDescription = "Фото події",
                            contentScale = ContentScale.Crop,
                            modifier = Modifier
                                .size(width = 160.dp, height = 110.dp)
                                .clip(RoundedCornerShape(12.dp)),
                        )
                    }
                }
            }

            event.videoUrl?.let { video ->
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth().clickable { uriHandler.openUri(video) },
                ) {
                    Icon(Icons.Filled.PlayCircle, contentDescription = null, tint = Accent)
                    Text(
                        "Дивитися відео",
                        modifier = Modifier.padding(start = 6.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        color = Accent,
                    )
                }
            }

            if (event.links.isNotEmpty()) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    event.links.forEach { link ->
                        Surface(
                            onClick = { uriHandler.openUri(link) },
                            shape = RoundedCornerShape(50),
                            color = MaterialTheme.colorScheme.surfaceVariant,
                        ) {
                            Text(
                                linkLabel(link),
                                style = MaterialTheme.typography.labelMedium,
                                modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                            )
                        }
                    }
                }
            }

            event.organizerId?.let { organizerId ->
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth().clickable { onOpenOrganizer(organizerId) },
                ) {
                    Icon(Icons.Filled.Person, contentDescription = null, tint = Accent)
                    Text(
                        "Організатор — профіль і підписка",
                        modifier = Modifier.padding(start = 6.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        color = Accent,
                    )
                }
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.fillMaxWidth().clickable { onMessageOrganizer(organizerId) },
                ) {
                    Icon(Icons.Filled.MailOutline, contentDescription = null, tint = Accent)
                    Text(
                        "Написати організатору",
                        modifier = Modifier.padding(start = 6.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        color = Accent,
                    )
                }
            }
            event.ageRating?.let { ar ->
                Text(
                    "Подія $ar+. Вік перевіряється за датою народження в Налаштуваннях.",
                    style = MaterialTheme.typography.bodySmall,
                    color = Muted,
                )
            }
            if (event.salesClosed && !past) {
                Text("Продаж квитків призупинено", color = MaterialTheme.colorScheme.error)
            }
            Row(
                Modifier.fillMaxWidth().padding(top = 4.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    if (event.isFree) "Безкоштовно" else Format.price(event.price, event.currency),
                    style = MaterialTheme.typography.headlineSmall,
                    color = Accent,
                    fontWeight = FontWeight.Bold,
                )
                Text("Залишилось ${event.ticketsLeft}", color = Muted, style = MaterialTheme.typography.bodyMedium)
            }
            Button(
                onClick = { if (event.isFree) onReserve(event.id) else onBuy(event.id) },
                enabled = !past,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(
                    when {
                        past -> "Подія завершилась"
                        event.isFree -> "Отримати квиток"
                        else -> "Купити квиток"
                    },
                )
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                androidx.compose.material3.OutlinedButton(onClick = { share() }, modifier = Modifier.weight(1f)) {
                    Icon(Icons.Filled.Share, contentDescription = null)
                    Text("  Поділитися")
                }
                androidx.compose.material3.OutlinedButton(onClick = { addToCalendar() }, modifier = Modifier.weight(1f)) {
                    Icon(Icons.Filled.Event, contentDescription = null)
                    Text("  В календар")
                }
            }
            OutlinedButton(onClick = { showReport = true }, modifier = Modifier.fillMaxWidth()) {
                Text("⚑ Поскаржитися")
            }
            ReviewsSection(reviewsVm, event.id, reviewsOpen = past || event.startsAt == null)

            if (similar.isNotEmpty()) {
                Text("Схожі події", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                LazyRow(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    items(similar, key = { it.id }) { ev ->
                        Box(Modifier.width(280.dp)) {
                            com.underground.app.ui.components.EventCard(ev) { onOpenEvent(ev.id) }
                        }
                    }
                }
            }
        }
    }

    if (showReport) {
        AlertDialog(
            onDismissRequest = { showReport = false },
            title = { Text("Поскаржитися на подію") },
            text = {
                OutlinedTextField(
                    value = reportReason,
                    onValueChange = { reportReason = it },
                    label = { Text("Причина") },
                    modifier = Modifier.fillMaxWidth(),
                )
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        val reason = reportReason.trim()
                        showReport = false
                        reportReason = ""
                        scope.launch {
                            try {
                                com.underground.app.UndergroundApp.container.authApi.createReport(
                                    com.underground.app.data.CreateReportReq("event", event.id, reason),
                                )
                                android.widget.Toast.makeText(context, "Скаргу надіслано", android.widget.Toast.LENGTH_SHORT).show()
                            } catch (e: Exception) {
                                android.widget.Toast.makeText(context, "Не вдалося надіслати скаргу", android.widget.Toast.LENGTH_SHORT).show()
                            }
                        }
                    },
                    enabled = reportReason.isNotBlank(),
                ) { Text("Надіслати") }
            },
            dismissButton = { TextButton(onClick = { showReport = false }) { Text("Скасувати") } },
        )
    }
}

private fun linkLabel(url: String): String = when {
    "instagram" in url -> "Instagram"
    "facebook" in url || "fb.com" in url -> "Facebook"
    "t.me" in url || "telegram" in url -> "Telegram"
    "youtube" in url || "youtu.be" in url -> "YouTube"
    "tiktok" in url -> "TikTok"
    else -> "Сайт"
}
