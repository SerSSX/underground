package com.underground.app.ui.events

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.underground.app.data.EventDto
import com.underground.app.ui.components.EventCard
import com.underground.app.ui.theme.Muted

private fun statusLabel(event: EventDto): String = buildList {
    when (event.status) {
        "hidden" -> add("Сховано")
        "cancelled" -> add("Скасовано")
        else -> add("Опубліковано")
    }
    if (event.salesClosed && event.status != "cancelled") add("стоп продажів")
}.joinToString(" · ")

@Composable
fun MyEventsScreen(
    vm: MyEventsViewModel,
    onCreate: () -> Unit,
    onOpen: (String) -> Unit,
    onManage: (String) -> Unit,
) {
    val events by vm.events.collectAsState()
    val delegated by vm.delegatedEvents.collectAsState()
    val loading by vm.loading.collectAsState()

    Column(Modifier.fillMaxSize()) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Мої події", style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
            // Maker tools appear progressively — only once the user has own events.
            if (events.isNotEmpty()) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    ScanTicketsButton()
                    FilledTonalButton(onClick = onCreate) {
                        Icon(Icons.Filled.Add, contentDescription = null)
                        Spacer(Modifier.width(6.dp))
                        Text("Створити")
                    }
                }
            }
        }
        when {
            loading -> Box(Modifier.weight(1f).fillMaxWidth(), Alignment.Center) { CircularProgressIndicator() }
            events.isEmpty() && delegated.isEmpty() -> Box(Modifier.weight(1f).fillMaxWidth(), Alignment.Center) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("Ще немає подій", color = Muted)
                    Spacer(Modifier.height(12.dp))
                    Button(onClick = onCreate) { Text("Створити першу подію") }
                }
            }
            else -> LazyColumn(
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 16.dp),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                items(events, key = { it.id }) { event ->
                    Column {
                        EventCard(event) { onOpen(event.id) }
                        Row(
                            Modifier.fillMaxWidth().padding(top = 6.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text(statusLabel(event), style = MaterialTheme.typography.labelMedium, color = Muted)
                            FilledTonalButton(onClick = { onManage(event.id) }) {
                                Icon(Icons.Filled.Settings, contentDescription = null)
                                Spacer(Modifier.width(6.dp))
                                Text("Керувати")
                            }
                        }
                    }
                }
                if (delegated.isNotEmpty()) {
                    item(key = "delegated-header") {
                        Text(
                            "Події, де ти контролер",
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.Bold,
                            modifier = Modifier.padding(top = 8.dp),
                        )
                    }
                    items(delegated, key = { "staff-${it.id}" }) { event ->
                        Column {
                            EventCard(event) { onOpen(event.id) }
                            EventScanButton(event.id, Modifier.fillMaxWidth().padding(top = 6.dp))
                        }
                    }
                }
            }
        }
    }
}
