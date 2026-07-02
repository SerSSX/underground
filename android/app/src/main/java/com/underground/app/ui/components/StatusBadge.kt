package com.underground.app.ui.components

import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import com.underground.app.ui.theme.Danger
import com.underground.app.ui.theme.Muted
import com.underground.app.ui.theme.Ok

/**
 * Localized status pill (AND-1/AND-2). Colour + a leading glyph so the state is conveyed
 * without relying on colour alone (colourblind / TalkBack safe), using the shared palette.
 */
@Composable
fun StatusBadge(status: String) {
    val (label, color, glyph) = when (status) {
        "active" -> Triple("Активний", Ok, "✓")
        "pending_claim" -> Triple("Очікує отримання", Muted, "⏳")
        "used" -> Triple("Використаний", Muted, "•")
        "returned" -> Triple("Повернений", Danger, "✕")
        "published" -> Triple("Опубліковано", Ok, "✓")
        "hidden" -> Triple("Сховано", Muted, "◐")
        "cancelled" -> Triple("Скасовано", Danger, "✕")
        else -> Triple(status, Muted, "•")
    }
    Surface(color = color.copy(alpha = 0.18f), shape = RoundedCornerShape(50)) {
        Text(
            "$glyph $label",
            style = MaterialTheme.typography.labelMedium,
            color = color,
            modifier = Modifier
                .padding(horizontal = 10.dp, vertical = 3.dp)
                .semantics { contentDescription = "Статус: $label" },
        )
    }
}
