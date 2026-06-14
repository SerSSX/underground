package com.underground.app.ui.mine

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Tab
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import com.underground.app.ui.events.MyEventsScreen
import com.underground.app.ui.events.MyEventsViewModel
import com.underground.app.ui.tickets.MyTicketsScreen
import com.underground.app.ui.tickets.TicketsViewModel

@Composable
fun MineScreen(
    ticketsVm: TicketsViewModel,
    myEventsVm: MyEventsViewModel,
    onCreate: () -> Unit,
    onOpenEvent: (String) -> Unit,
    onOpenTicket: (String) -> Unit,
    onManageEvent: (String) -> Unit,
) {
    var tab by remember { mutableIntStateOf(0) }
    Column(Modifier.fillMaxSize()) {
        TabRow(selectedTabIndex = tab) {
            Tab(selected = tab == 0, onClick = { tab = 0 }, text = { Text("Квитки") })
            Tab(selected = tab == 1, onClick = { tab = 1 }, text = { Text("Мої події") })
        }
        when (tab) {
            0 -> MyTicketsScreen(ticketsVm, onOpenTicket)
            else -> MyEventsScreen(myEventsVm, onCreate, onOpenEvent, onManageEvent)
        }
    }
}
