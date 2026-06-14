package com.underground.app.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.underground.app.ui.theme.Muted

/** Reusable loading / empty / error states (AND-1) so every screen feels consistent. */
@Composable
fun LoadingState() {
    Box(Modifier.fillMaxSize(), Alignment.Center) { CircularProgressIndicator() }
}

@Composable
fun EmptyState(emoji: String, title: String, subtitle: String? = null) {
    Box(Modifier.fillMaxSize().padding(32.dp), Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(emoji, style = MaterialTheme.typography.headlineMedium)
            Text(title, style = MaterialTheme.typography.titleMedium, textAlign = TextAlign.Center)
            subtitle?.let { Text(it, color = Muted, textAlign = TextAlign.Center) }
        }
    }
}

@Composable
fun ErrorState(message: String, onRetry: () -> Unit) {
    Box(Modifier.fillMaxSize().padding(32.dp), Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Text("⚠", style = MaterialTheme.typography.headlineMedium)
            Text(message, color = Muted, textAlign = TextAlign.Center)
            OutlinedButton(onClick = onRetry) { Text("Спробувати ще") }
        }
    }
}
