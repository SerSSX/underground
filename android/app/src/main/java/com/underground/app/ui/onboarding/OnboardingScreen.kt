package com.underground.app.ui.onboarding

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.toMutableStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.underground.app.util.EVENT_CATEGORIES

/** Shared interests editor: onboarding (F-10b) and «Мої інтереси» in the profile (P0-4). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun InterestsEditor(
    title: String,
    initialCategories: List<String>,
    initialCity: String,
    busy: Boolean,
    skipLabel: String?,
    onDone: (categories: List<String>, city: String?) -> Unit,
    onSkip: () -> Unit = {},
) {
    val selected = remember(initialCategories) { initialCategories.toMutableStateList() }
    var city by remember(initialCity) { mutableStateOf(initialCity) }

    Column(Modifier.fillMaxSize().padding(24.dp)) {
        Spacer(Modifier.height(24.dp))
        Text(title, style = MaterialTheme.typography.headlineMedium)
        Spacer(Modifier.height(4.dp))
        Text(
            "Обери інтереси й місто — підкажемо події під тебе.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))

        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                EVENT_CATEGORIES.forEach { category ->
                    FilterChip(
                        selected = category in selected,
                        onClick = {
                            if (category in selected) selected.remove(category) else selected.add(category)
                        },
                        label = { Text(category) },
                    )
                }
            }
            Spacer(Modifier.height(16.dp))
            com.underground.app.ui.components.CityField(value = city, onPick = { city = it })
        }

        Spacer(Modifier.height(12.dp))
        Button(
            onClick = { onDone(selected.toList(), city) },
            enabled = !busy,
            modifier = Modifier.fillMaxWidth(),
        ) {
            Text(if (selected.isEmpty()) "Зберегти" else "Зберегти (${selected.size})")
        }
        if (skipLabel != null) {
            TextButton(
                onClick = onSkip,
                enabled = !busy,
                modifier = Modifier.align(Alignment.CenterHorizontally),
            ) {
                Text(skipLabel)
            }
        }
    }
}

/** Post-registration onboarding: pick interest categories + home city (F-10b). */
@Composable
fun OnboardingScreen(
    busy: Boolean,
    onDone: (categories: List<String>, city: String?) -> Unit,
) {
    InterestsEditor(
        title = "Що тобі цікаво?",
        initialCategories = emptyList(),
        initialCity = "Київ",
        busy = busy,
        skipLabel = "Пропустити",
        onDone = onDone,
        onSkip = { onDone(emptyList(), null) },
    )
}
