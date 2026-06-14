package com.underground.app.ui.tickets

import android.widget.Toast
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.underground.app.data.TicketDto
import com.underground.app.ui.theme.Muted
import com.underground.app.util.Format
import com.underground.app.util.qrBitmap

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ETicketScreen(vm: TicketsViewModel, ticket: TicketDto?, onBack: () -> Unit) {
    val context = LocalContext.current
    var showTransfer by remember { mutableStateOf(false) }
    var showDispute by remember { mutableStateOf(false) }
    var email by remember { mutableStateOf("") }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Квиток") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
                    }
                },
            )
        },
    ) { inner ->
        if (ticket == null) {
            Box(Modifier.fillMaxSize().padding(inner), Alignment.Center) { Text("Квиток не знайдено") }
            return@Scaffold
        }
        Column(
            modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(inner).padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Text(
                ticket.eventTitle,
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.Bold,
                textAlign = TextAlign.Center,
            )
            Text(
                listOfNotNull(ticket.eventCity, Format.eventDate(ticket.eventStartsAt)).joinToString(" · "),
                color = Muted,
            )
            ticket.eventVenueAddress?.let { Text(it, color = Muted, textAlign = TextAlign.Center) }

            val bitmap = remember(ticket.qrToken) { qrBitmap(ticket.qrToken) }
            Surface(color = Color.White, shape = RoundedCornerShape(16.dp)) {
                Image(
                    bitmap = bitmap.asImageBitmap(),
                    contentDescription = "QR квитка",
                    modifier = Modifier.size(280.dp).padding(16.dp),
                )
            }
            Text(ticket.qrToken, style = MaterialTheme.typography.bodySmall, color = Muted)
            Text("Покажіть QR на вході", style = MaterialTheme.typography.labelLarge)

            if (ticket.status == "active") {
                OutlinedButton(onClick = { showTransfer = true }, modifier = Modifier.fillMaxWidth()) {
                    Text("Подарувати")
                }
                OutlinedButton(
                    onClick = {
                        vm.returnTicket(
                            ticket.id,
                            onDone = {
                                Toast.makeText(context, "Квиток повернено", Toast.LENGTH_SHORT).show()
                                onBack()
                            },
                            onError = { msg -> Toast.makeText(context, msg, Toast.LENGTH_SHORT).show() },
                        )
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Повернути квиток")
                }
            } else {
                Text("Статус: ${ticket.status}", color = Muted)
            }

            // Dispute the paid order behind this ticket (parity with web): possible while the
            // organizer hasn't been paid (escrow held|clearing); shows status once opened.
            if (ticket.orderId != null && (ticket.escrowStatus == "held" || ticket.escrowStatus == "clearing")) {
                OutlinedButton(onClick = { showDispute = true }, modifier = Modifier.fillMaxWidth()) {
                    Text("Поскаржитися на подію")
                }
                Text(
                    "Щось пішло не так? Заморозимо кошти до рішення модерації.",
                    style = MaterialTheme.typography.bodySmall,
                    color = Muted,
                    textAlign = TextAlign.Center,
                )
            } else if (ticket.escrowStatus == "frozen") {
                Text("⏳ Спір на розгляді", color = Muted)
            }
        }
    }

    if (showDispute) {
        AlertDialog(
            onDismissRequest = { showDispute = false },
            title = { Text("Відкрити спір?") },
            text = { Text("Кошти заморозяться до рішення модерації, виплата організатору призупиниться.") },
            confirmButton = {
                TextButton(onClick = {
                    showDispute = false
                    val orderId = ticket?.orderId
                    if (orderId != null) {
                        vm.dispute(
                            orderId,
                            onDone = {
                                Toast.makeText(context, "Спір відкрито", Toast.LENGTH_SHORT).show()
                                onBack()
                            },
                            onError = { msg -> Toast.makeText(context, msg, Toast.LENGTH_SHORT).show() },
                        )
                    }
                }) { Text("Відкрити спір") }
            },
            dismissButton = { TextButton(onClick = { showDispute = false }) { Text("Скасувати") } },
        )
    }

    if (showTransfer) {
        AlertDialog(
            onDismissRequest = { showTransfer = false },
            title = { Text("Подарувати квиток") },
            text = {
                OutlinedTextField(
                    value = email,
                    onValueChange = { email = it },
                    label = { Text("Email отримувача") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    showTransfer = false
                    val t = ticket
                    if (t != null) {
                        vm.transferTicket(
                            t.id,
                            email,
                            onDone = {
                                Toast.makeText(context, "Квиток подаровано", Toast.LENGTH_SHORT).show()
                                onBack()
                            },
                            onError = { msg -> Toast.makeText(context, msg, Toast.LENGTH_SHORT).show() },
                        )
                    }
                }) { Text("Подарувати") }
            },
            dismissButton = { TextButton(onClick = { showTransfer = false }) { Text("Скасувати") } },
        )
    }
}
