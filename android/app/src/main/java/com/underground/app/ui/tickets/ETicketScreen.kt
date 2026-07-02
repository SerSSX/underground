package com.underground.app.ui.tickets

import android.content.Intent
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
import androidx.compose.material3.CircularProgressIndicator
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
import androidx.compose.runtime.LaunchedEffect
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
import com.underground.app.BuildConfig
import com.underground.app.data.TicketDto
import com.underground.app.ui.theme.Muted
import com.underground.app.ui.theme.Ok
import com.underground.app.util.Format
import kotlinx.coroutines.delay
import com.underground.app.util.qrBitmap

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ETicketScreen(vm: TicketsViewModel, ticket: TicketDto?, onBack: () -> Unit) {
    val context = LocalContext.current
    var showTransfer by remember { mutableStateOf(false) }
    var showDispute by remember { mutableStateOf(false) }
    var showGiftLink by remember { mutableStateOf(false) }
    var email by remember { mutableStateOf("") }
    var giftEmail by remember { mutableStateOf("") }
    var giftLink by remember { mutableStateOf<String?>(null) }
    var giftBusy by remember { mutableStateOf(false) }

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

            // Active tickets show a signed, short-TTL QR that auto-refreshes (BR-023); the raw
            // token is never displayed (a screenshot of a stale QR can't be replayed at the door).
            if (ticket.status == "active") {
                var qrToken by remember(ticket.id) { mutableStateOf<String?>(null) }
                LaunchedEffect(ticket.id) {
                    while (true) {
                        val t = vm.fetchSignedQr(ticket.id)
                        if (t != null) {
                            qrToken = t
                            delay(100_000) // refresh before the 120s TTL expires
                        } else {
                            delay(5_000) // transient failure — retry shortly
                        }
                    }
                }
                Surface(color = Color.White, shape = RoundedCornerShape(16.dp)) {
                    val t = qrToken
                    if (t != null) {
                        val bitmap = remember(t) { qrBitmap(t) }
                        Image(
                            bitmap = bitmap.asImageBitmap(),
                            contentDescription = "QR квитка",
                            modifier = Modifier.size(280.dp).padding(16.dp),
                        )
                    } else {
                        Box(Modifier.size(280.dp), contentAlignment = Alignment.Center) {
                            CircularProgressIndicator()
                        }
                    }
                }
                Text("Покажіть QR на вході", style = MaterialTheme.typography.labelLarge)
            }

            if (ticket.status == "active") {
                OutlinedButton(onClick = { showGiftLink = true }, modifier = Modifier.fillMaxWidth()) {
                    Text("Подарувати лінком")
                }
                OutlinedButton(onClick = { showTransfer = true }, modifier = Modifier.fillMaxWidth()) {
                    Text("Подарувати другу (email)")
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
            } else if (ticket.status == "pending_claim") {
                Text("⏳ Очікує отримання", color = Muted)
                OutlinedButton(
                    onClick = {
                        vm.revokeGift(
                            ticket.id,
                            onDone = {
                                Toast.makeText(context, "Подарунок скасовано", Toast.LENGTH_SHORT).show()
                                onBack()
                            },
                            onError = { msg -> Toast.makeText(context, msg, Toast.LENGTH_SHORT).show() },
                        )
                    },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text("Скасувати подарунок")
                }
            } else {
                Text("Статус: ${ticket.status}", color = Muted)
            }

            // Proactive buyer-protection reassurance while funds are still in escrow (TASK-199-B).
            if (ticket.escrowStatus == "held" || ticket.escrowStatus == "clearing") {
                Text(
                    "🛡 Кошти захищені в escrow до завершення події",
                    color = Ok,
                    style = MaterialTheme.typography.bodySmall,
                    textAlign = TextAlign.Center,
                )
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

    if (showGiftLink) {
        val link = giftLink
        AlertDialog(
            onDismissRequest = {
                if (!giftBusy) {
                    showGiftLink = false
                    giftLink = null
                    giftEmail = ""
                }
            },
            title = { Text("Подарувати лінком") },
            text = {
                if (link == null) {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(
                            "Створимо одноразовий лінк. Надішли його будь-кому — навіть без акаунта.",
                            style = MaterialTheme.typography.bodySmall,
                            color = Muted,
                        )
                        OutlinedTextField(
                            value = giftEmail,
                            onValueChange = { giftEmail = it },
                            label = { Text("Email отримувача") },
                            singleLine = true,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                        )
                    }
                } else {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text("Лінк створено. Квиток очікує отримання.", color = Muted)
                        Text(link, style = MaterialTheme.typography.bodySmall)
                    }
                }
            },
            confirmButton = {
                if (link == null) {
                    TextButton(
                        enabled = giftEmail.isNotBlank() && !giftBusy,
                        onClick = {
                            val t = ticket ?: return@TextButton
                            giftBusy = true
                            vm.giftTicket(
                                t.id,
                                giftEmail,
                                onDone = { resp ->
                                    giftBusy = false
                                    giftLink = "${BuildConfig.WEB_BASE_URL}/g/${resp.claimToken}"
                                },
                                onError = { msg ->
                                    giftBusy = false
                                    Toast.makeText(context, msg, Toast.LENGTH_SHORT).show()
                                },
                            )
                        },
                    ) { Text(if (giftBusy) "Створення…" else "Створити лінк") }
                } else {
                    TextButton(onClick = {
                        val send = Intent(Intent.ACTION_SEND).apply {
                            type = "text/plain"
                            putExtra(Intent.EXTRA_TEXT, "Дарую тобі квиток: $link")
                        }
                        context.startActivity(Intent.createChooser(send, "Поділитися лінком"))
                    }) { Text("Поділитися") }
                }
            },
            dismissButton = {
                if (link == null) {
                    TextButton(
                        enabled = !giftBusy,
                        onClick = { showGiftLink = false; giftEmail = "" },
                    ) { Text("Скасувати") }
                } else {
                    TextButton(onClick = {
                        showGiftLink = false
                        giftLink = null
                        giftEmail = ""
                        onBack()
                    }) { Text("Готово") }
                }
            },
        )
    }
}
