package com.underground.app.ui.afisha

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.underground.app.ui.components.BrandHeader
import com.underground.app.ui.components.CategoryChips
import com.underground.app.ui.components.EventCard

@Composable
fun AfishaScreen(vm: AfishaViewModel, onOpen: (String) -> Unit) {
    val state by vm.state.collectAsState()
    val category by vm.category.collectAsState()
    val mode by vm.mode.collectAsState()
    val savedIds by vm.savedIds.collectAsState()

    Column(Modifier.fillMaxSize()) {
        BrandHeader()
        // Discovery first, catalog second.
        Row(
            Modifier.fillMaxWidth().padding(horizontal = 16.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            FilterChip(
                selected = mode == AfishaMode.ForYou,
                onClick = { vm.setMode(AfishaMode.ForYou) },
                label = { Text("Для тебе") },
            )
            FilterChip(
                selected = mode == AfishaMode.All,
                onClick = { vm.setMode(AfishaMode.All) },
                label = { Text("Усі події") },
            )
        }
        when (val s = state) {
            AfishaUiState.Loading -> Center { CircularProgressIndicator() }
            AfishaUiState.Empty -> Center {
                Column(horizontalAlignment = Alignment.CenterHorizontally, verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text("🎭", style = MaterialTheme.typography.headlineMedium)
                    Text(
                        if (mode == AfishaMode.ForYou) {
                            "Поки нічого порекомендувати. Обери інтереси й місто в профілі."
                        } else {
                            "Подій поки немає"
                        },
                    )
                }
            }
            is AfishaUiState.Error -> Center {
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text("Помилка: ${s.message}")
                    Button(onClick = vm::load) { Text("Повторити") }
                }
            }
            is AfishaUiState.Success -> {
                if (mode == AfishaMode.All) {
                    val categories = remember(s.events) { s.events.map { it.category }.distinct() }
                    CategoryChips(categories, category, vm::selectCategory)
                }
                val list = if (mode == AfishaMode.All) filterByCategory(s.events, category) else s.events
                LazyColumn(
                    modifier = Modifier.fillMaxWidth(),
                    contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 16.dp),
                    verticalArrangement = Arrangement.spacedBy(16.dp),
                ) {
                    items(list, key = { it.id }) { event ->
                        EventCard(
                            event = event,
                            saved = event.id in savedIds,
                            reason = if (mode == AfishaMode.ForYou) event.reason else null,
                            onToggleSave = { vm.toggleSave(event.id) },
                            onClick = { onOpen(event.id) },
                        )
                    }
                }
            }
            is AfishaUiState.Sections -> LazyColumn(
                modifier = Modifier.fillMaxWidth(),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 12.dp, bottom = 16.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                s.sections.forEach { section ->
                    item(key = "hdr-${section.key}") {
                        Text(
                            section.title,
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Bold,
                        )
                    }
                    items(section.events, key = { "${section.key}-${it.id}" }) { event ->
                        EventCard(
                            event = event,
                            saved = event.id in savedIds,
                            reason = event.reason,
                            onToggleSave = { vm.toggleSave(event.id) },
                            onClick = { onOpen(event.id) },
                        )
                    }
                }
            }
        }
    }
}

/** Fills the remaining column space and centres its content. */
@Composable
private fun androidx.compose.foundation.layout.ColumnScope.Center(content: @Composable () -> Unit) {
    Box(Modifier.weight(1f).fillMaxWidth(), contentAlignment = Alignment.Center) { content() }
}
