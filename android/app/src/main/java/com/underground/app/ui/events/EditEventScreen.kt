package com.underground.app.ui.events

import android.widget.Toast
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import com.underground.app.data.EventDto
import com.underground.app.data.UpdateEventReq
import com.underground.app.ui.theme.Muted

/** Organizer's mobile admin for one event: edit, hide/publish, pause sales, cancel. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EditEventScreen(
    vm: MyEventsViewModel,
    event: EventDto?,
    onDone: () -> Unit,
    onBack: () -> Unit,
) {
    val context = LocalContext.current
    if (event == null) {
        Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) { Text("Подію не знайдено") }
        return
    }

    var title by remember { mutableStateOf(event.title) }
    var description by remember { mutableStateOf(event.description.orEmpty()) }
    var city by remember { mutableStateOf(event.city.orEmpty()) }
    var venue by remember { mutableStateOf(event.venueAddress.orEmpty()) }
    var price by remember { mutableStateOf(event.price) }
    var videoUrl by remember { mutableStateOf(event.videoUrl.orEmpty()) }
    var links by remember { mutableStateOf(event.links.joinToString(", ")) }
    var hidden by remember { mutableStateOf(event.status == "hidden") }
    var salesClosed by remember { mutableStateOf(event.salesClosed) }
    var confirmCancel by remember { mutableStateOf(false) }

    val toast = { msg: String -> Toast.makeText(context, msg, Toast.LENGTH_SHORT).show() }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Керування подією") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
                    }
                },
            )
        },
    ) { inner ->
        Column(
            modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(inner).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            OutlinedTextField(title, { title = it }, label = { Text("Назва") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(description, { description = it }, label = { Text("Опис") }, minLines = 3, modifier = Modifier.fillMaxWidth())
            com.underground.app.ui.components.CityField(value = city, onPick = { city = it })
            OutlinedTextField(venue, { venue = it }, label = { Text("Адреса") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            if (!event.costType.equals("free", ignoreCase = true)) {
                OutlinedTextField(
                    value = price,
                    onValueChange = { v -> price = v.filter { it.isDigit() || it == '.' } },
                    label = { Text("Ціна") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            OutlinedTextField(videoUrl, { videoUrl = it }, label = { Text("Відео (YouTube URL)") }, singleLine = true, modifier = Modifier.fillMaxWidth())
            OutlinedTextField(links, { links = it }, label = { Text("Соцмережі (через кому)") }, modifier = Modifier.fillMaxWidth())

            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.padding(end = 8.dp)) {
                    Text("Сховати з афіші")
                    Text("Подію бачите лише ви", style = MaterialTheme.typography.bodySmall, color = Muted)
                }
                Switch(checked = hidden, onCheckedChange = { hidden = it })
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.padding(end = 8.dp)) {
                    Text("Стоп продажів")
                    Text("Квитки тимчасово не продаються", style = MaterialTheme.typography.bodySmall, color = Muted)
                }
                Switch(checked = salesClosed, onCheckedChange = { salesClosed = it })
            }

            Button(
                onClick = {
                    vm.update(
                        event.id,
                        UpdateEventReq(
                            title = title.trim(),
                            description = description.ifBlank { null },
                            city = city.ifBlank { null },
                            venueAddress = venue.ifBlank { null },
                            price = price.ifBlank { null },
                            videoUrl = videoUrl.trim().ifBlank { null },
                            links = links.split(',').map { it.trim() }.filter { it.isNotEmpty() },
                            status = if (hidden) "hidden" else "published",
                            salesClosed = salesClosed,
                        ),
                        onError = toast,
                    )
                    onDone()
                },
                enabled = title.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Зберегти")
            }
            OutlinedButton(
                onClick = { confirmCancel = true },
                colors = androidx.compose.material3.ButtonDefaults.outlinedButtonColors(
                    contentColor = MaterialTheme.colorScheme.error,
                ),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text("Скасувати подію")
            }
        }
    }

    if (confirmCancel) {
        AlertDialog(
            onDismissRequest = { confirmCancel = false },
            title = { Text("Скасувати подію?") },
            text = { Text("Подія зникне з афіші назавжди. Цю дію не можна відмінити.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmCancel = false
                    vm.cancel(event.id, onError = toast)
                    onDone()
                }) { Text("Так, скасувати") }
            },
            dismissButton = { TextButton(onClick = { confirmCancel = false }) { Text("Ні") } },
        )
    }
}
