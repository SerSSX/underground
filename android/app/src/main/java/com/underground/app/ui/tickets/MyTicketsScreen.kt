package com.underground.app.ui.tickets

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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ConfirmationNumber
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
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
import com.underground.app.data.TicketDto
import com.underground.app.ui.components.StatusBadge
import com.underground.app.ui.theme.Accent
import com.underground.app.ui.theme.Muted
import com.underground.app.util.Format

@Composable
fun MyTicketsScreen(vm: TicketsViewModel, onOpen: (String) -> Unit) {
    val tickets by vm.tickets.collectAsState()
    val loading by vm.loading.collectAsState()

    when {
        loading -> Box(Modifier.fillMaxSize(), Alignment.Center) { CircularProgressIndicator() }
        tickets.isEmpty() -> Box(Modifier.fillMaxSize().padding(32.dp), Alignment.Center) {
            Text(
                "Ще немає квитків.\nОтримай квиток на безкоштовній події.",
                color = Muted,
            )
        }
        else -> LazyColumn(
            contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            items(tickets, key = { it.id }) { ticket -> TicketRow(ticket) { onOpen(ticket.id) } }
        }
    }
}

@Composable
private fun TicketRow(ticket: TicketDto, onClick: () -> Unit) {
    Card(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(Icons.Filled.ConfirmationNumber, contentDescription = null, tint = Accent)
            Column(Modifier.weight(1f)) {
                Text(ticket.eventTitle, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold, maxLines = 1)
                Text(
                    listOfNotNull(ticket.eventCity, Format.eventDate(ticket.eventStartsAt)).joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall,
                    color = Muted,
                    maxLines = 1,
                )
            }
            StatusBadge(ticket.status)
        }
    }
}
