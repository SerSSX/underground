package com.underground.app.ui.detail

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import android.widget.Toast
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Flag
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.underground.app.UndergroundApp
import com.underground.app.data.CreateReportReq
import com.underground.app.ui.theme.Accent
import com.underground.app.ui.theme.Muted
import kotlinx.coroutines.launch
import java.util.Locale

/** Reviews block on the event detail screen: aggregate, composer, list (F-13b).
 *  The composer is available only once the event has started ([reviewsOpen]). */
@Composable
fun ReviewsSection(vm: ReviewsViewModel, eventId: String, reviewsOpen: Boolean = true) {
    LaunchedEffect(eventId) { vm.load(eventId) }
    val state by vm.state.collectAsState()
    var composing by remember { mutableStateOf(false) }
    var rating by remember { mutableIntStateOf(0) }
    var text by remember { mutableStateOf("") }
    var reportReviewId by remember { mutableStateOf<String?>(null) }
    var reportReason by remember { mutableStateOf("") }
    val context = LocalContext.current
    val scope = rememberCoroutineScope()

    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text("Відгуки", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            state.data.avgRating?.let { avg ->
                Icon(Icons.Filled.Star, contentDescription = null, tint = Accent)
                Text(
                    String.format(Locale.US, "%.1f · %d", avg, state.data.count),
                    style = MaterialTheme.typography.bodyMedium,
                    color = Muted,
                )
            }
        }

        if (!reviewsOpen) {
            Text(
                "Відгуки відкриються після початку події",
                style = MaterialTheme.typography.bodyMedium,
                color = Muted,
            )
        } else if (!composing) {
            TextButton(onClick = { composing = true }) { Text("Залишити відгук") }
        } else {
            Row {
                (1..5).forEach { star ->
                    IconButton(onClick = { rating = star }) {
                        Icon(
                            if (star <= rating) Icons.Filled.Star else Icons.Filled.StarBorder,
                            contentDescription = "Оцінка $star",
                            tint = if (star <= rating) Accent else Muted,
                        )
                    }
                }
            }
            OutlinedTextField(
                value = text,
                onValueChange = { text = it },
                label = { Text("Ваш відгук (необов'язково)") },
                modifier = Modifier.fillMaxWidth(),
            )
            state.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
            Button(
                onClick = {
                    vm.post(eventId, rating, text) {
                        composing = false
                        rating = 0
                        text = ""
                    }
                },
                enabled = rating in 1..5 && !state.sending,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Надіслати")
            }
        }

        state.data.reviews.forEach { review ->
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
            ) {
                Column(Modifier.padding(12.dp)) {
                    Row(
                        Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Text(review.authorName, fontWeight = FontWeight.SemiBold)
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(Icons.Filled.Star, contentDescription = null, tint = Accent)
                            Text("${review.rating}", color = Muted)
                            IconButton(onClick = { reportReason = ""; reportReviewId = review.id }) {
                                Icon(Icons.Filled.Flag, contentDescription = "Поскаржитися на відгук", tint = Muted)
                            }
                        }
                    }
                    review.text?.let {
                        Spacer(Modifier.height(4.dp))
                        Text(it, style = MaterialTheme.typography.bodyMedium)
                    }
                }
            }
        }
    }

    reportReviewId?.let { reviewId ->
        AlertDialog(
            onDismissRequest = { reportReviewId = null },
            title = { Text("Поскаржитися на відгук") },
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
                    enabled = reportReason.isNotBlank(),
                    onClick = {
                        val reason = reportReason
                        reportReviewId = null
                        scope.launch {
                            try {
                                UndergroundApp.container.authApi.createReport(CreateReportReq("review", reviewId, reason))
                                Toast.makeText(context, "Скаргу надіслано", Toast.LENGTH_SHORT).show()
                            } catch (e: Exception) {
                                Toast.makeText(context, e.message ?: "Помилка", Toast.LENGTH_SHORT).show()
                            }
                        }
                    },
                ) { Text("Надіслати") }
            },
            dismissButton = { TextButton(onClick = { reportReviewId = null }) { Text("Скасувати") } },
        )
    }
}
