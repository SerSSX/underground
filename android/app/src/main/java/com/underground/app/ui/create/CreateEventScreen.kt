package com.underground.app.ui.create

import android.content.Intent
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import androidx.lifecycle.viewmodel.compose.viewModel
import com.underground.app.BuildConfig
import com.underground.app.data.CreateEventReq
import com.underground.app.ui.theme.Muted
import com.underground.app.util.EVENT_CATEGORIES
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Date
import java.util.Locale
import java.util.TimeZone

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun CreateEventScreen(onDone: () -> Unit, onBack: () -> Unit, vm: CreateEventViewModel = viewModel()) {
    var title by remember { mutableStateOf("") }
    var description by remember { mutableStateOf("") }
    var category by remember { mutableStateOf(EVENT_CATEGORIES.first()) }
    var city by remember { mutableStateOf("Київ") }
    var venue by remember { mutableStateOf("") }
    var isFree by remember { mutableStateOf(false) }
    var price by remember { mutableStateOf("") }
    var tickets by remember { mutableStateOf("50") }
    var posterUrl by remember { mutableStateOf<String?>(null) }
    var galleryUrls by remember { mutableStateOf(listOf<String>()) }
    var videoUrl by remember { mutableStateOf("") }
    var links by remember { mutableStateOf("") }
    var ageRating by remember { mutableStateOf("") }
    var ageExpanded by remember { mutableStateOf(false) }

    val context = LocalContext.current
    val uploading by vm.uploading.collectAsState()
    fun uploadUri(uri: android.net.Uri, onUrl: (String) -> Unit) {
        val bytes = context.contentResolver.openInputStream(uri)?.use { it.readBytes() } ?: return
        val mime = context.contentResolver.getType(uri) ?: "image/jpeg"
        vm.upload(bytes, mime, onUrl)
    }
    val pickPoster = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        uri?.let { uploadUri(it) { url -> posterUrl = url } }
    }
    val pickGallery = rememberLauncherForActivityResult(
        ActivityResultContracts.PickMultipleVisualMedia(5),
    ) { uris ->
        uris.forEach { uri -> uploadUri(uri) { url -> galleryUrls = galleryUrls + url } }
    }

    var catExpanded by remember { mutableStateOf(false) }
    var showDate by remember { mutableStateOf(false) }
    var showTime by remember { mutableStateOf(false) }
    val dateState = rememberDatePickerState()
    val timeState = rememberTimePickerState(initialHour = 19, initialMinute = 0)

    val busy by vm.busy.collectAsState()
    val error by vm.error.collectAsState()
    val created by vm.created.collectAsState()

    val dateMillis = dateState.selectedDateMillis
    // Luma-mode: only a title (+ price/tickets) is required; the date is optional (TBA).
    val canSubmit = !busy && title.isNotBlank() && (isFree || price.isNotBlank()) &&
        tickets.toIntOrNull() != null

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Нова подія") },
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

        ExposedDropdownMenuBox(expanded = catExpanded, onExpandedChange = { catExpanded = !catExpanded }) {
            OutlinedTextField(
                value = category,
                onValueChange = {},
                readOnly = true,
                label = { Text("Категорія") },
                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = catExpanded) },
                modifier = Modifier.menuAnchor().fillMaxWidth(),
            )
            ExposedDropdownMenu(
                expanded = catExpanded,
                onDismissRequest = { catExpanded = false },
            ) {
                EVENT_CATEGORIES.forEach { c ->
                    DropdownMenuItem(text = { Text(c) }, onClick = { category = c; catExpanded = false })
                }
            }
        }

        com.underground.app.ui.components.CityField(value = city, onPick = { city = it })
        OutlinedTextField(venue, { venue = it }, label = { Text("Адреса") }, singleLine = true, modifier = Modifier.fillMaxWidth())

        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(onClick = { showDate = true }, modifier = Modifier.weight(1f)) {
                Text(dateMillis?.let(::formatDate) ?: "Дата")
            }
            OutlinedButton(onClick = { showTime = true }, modifier = Modifier.weight(1f)) {
                Text("%02d:%02d".format(timeState.hour, timeState.minute))
            }
        }

        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            Text("Безкоштовно")
            Switch(checked = isFree, onCheckedChange = { isFree = it })
        }
        if (!isFree) {
            OutlinedTextField(
                value = price,
                onValueChange = { v -> price = v.filter { it.isDigit() || it == '.' } },
                label = { Text("Ціна") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.fillMaxWidth(),
            )
        }
        OutlinedTextField(
            value = tickets,
            onValueChange = { v -> tickets = v.filter { it.isDigit() } },
            label = { Text("Кількість квитків") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            modifier = Modifier.fillMaxWidth(),
        )

        ExposedDropdownMenuBox(expanded = ageExpanded, onExpandedChange = { ageExpanded = !ageExpanded }) {
            OutlinedTextField(
                value = if (ageRating.isBlank()) "Без обмежень" else "$ageRating+",
                onValueChange = {},
                readOnly = true,
                label = { Text("Вікове обмеження") },
                trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded = ageExpanded) },
                modifier = Modifier.menuAnchor().fillMaxWidth(),
            )
            ExposedDropdownMenu(expanded = ageExpanded, onDismissRequest = { ageExpanded = false }) {
                listOf("", "13", "16", "18", "21").forEach { a ->
                    DropdownMenuItem(
                        text = { Text(if (a.isBlank()) "Без обмежень" else "$a+") },
                        onClick = { ageRating = a; ageExpanded = false },
                    )
                }
            }
        }

        Text("Медіа", style = MaterialTheme.typography.titleSmall)
        posterUrl?.let { url ->
            AsyncImage(
                model = url,
                contentDescription = "Постер",
                contentScale = ContentScale.Crop,
                modifier = Modifier.fillMaxWidth().height(160.dp).clip(RoundedCornerShape(12.dp)),
            )
        }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(
                onClick = { pickPoster.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) },
                enabled = !uploading,
                modifier = Modifier.weight(1f),
            ) {
                Text(if (posterUrl == null) "Постер-афіша" else "Замінити постер")
            }
            OutlinedButton(
                onClick = { pickGallery.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) },
                enabled = !uploading,
                modifier = Modifier.weight(1f),
            ) {
                Text("Галерея (${galleryUrls.size})")
            }
        }
        if (uploading) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                CircularProgressIndicator(modifier = Modifier.size(16.dp), strokeWidth = 2.dp)
                Text("  Завантаження фото…", style = MaterialTheme.typography.bodySmall)
            }
        }
        OutlinedTextField(
            value = videoUrl,
            onValueChange = { videoUrl = it },
            label = { Text("Відео (YouTube URL)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = links,
            onValueChange = { links = it },
            label = { Text("Соцмережі (посилання через кому)") },
            modifier = Modifier.fillMaxWidth(),
        )

        error?.let { Text(it, color = MaterialTheme.colorScheme.error) }

        Button(
            onClick = {
                val startsAt = dateMillis?.let { toIso(it, timeState.hour, timeState.minute) }
                vm.submit(
                    CreateEventReq(
                        title = title.trim(),
                        description = description.ifBlank { null },
                        category = category,
                        city = city.ifBlank { null },
                        venueAddress = venue.ifBlank { null },
                        startsAt = startsAt,
                        price = if (isFree) "0" else price.ifBlank { "0" },
                        ticketsTotal = tickets.toIntOrNull() ?: 0,
                        costType = if (isFree) "free" else "paid",
                        posterUrl = posterUrl,
                        videoUrl = videoUrl.trim().ifBlank { null },
                        galleryUrls = galleryUrls,
                        links = links.split(',').map { it.trim() }.filter { it.isNotEmpty() },
                        ageRating = ageRating.ifBlank { null },
                    ),
                )
            },
            enabled = canSubmit,
            modifier = Modifier.fillMaxWidth(),
        ) {
            if (busy) CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp) else Text("Опублікувати")
        }
        }
    }

    if (showDate) {
        DatePickerDialog(
            onDismissRequest = { showDate = false },
            confirmButton = { TextButton(onClick = { showDate = false }) { Text("OK") } },
            dismissButton = { TextButton(onClick = { showDate = false }) { Text("Скасувати") } },
        ) {
            DatePicker(state = dateState)
        }
    }
    if (showTime) {
        AlertDialog(
            onDismissRequest = { showTime = false },
            confirmButton = { TextButton(onClick = { showTime = false }) { Text("OK") } },
            text = { TimePicker(state = timeState) },
        )
    }

    // Post-publish viral hook: the moment the event is live, prompt to share its link.
    created?.let { ev ->
        val link = "${BuildConfig.WEB_BASE_URL}/events/${ev.id}"
        AlertDialog(
            onDismissRequest = onDone,
            title = { Text("🎉 Опубліковано!") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Поділись лінком, щоб прийшло більше людей.")
                    Text(link, style = MaterialTheme.typography.bodySmall, color = Muted)
                }
            },
            confirmButton = {
                TextButton(onClick = {
                    val send = Intent(Intent.ACTION_SEND).apply {
                        type = "text/plain"
                        putExtra(Intent.EXTRA_TEXT, "Приходь на «${ev.title}»: $link")
                    }
                    context.startActivity(Intent.createChooser(send, "Поділитися подією"))
                }) { Text("Поділитися") }
            },
            dismissButton = { TextButton(onClick = onDone) { Text("Готово") } },
        )
    }
}

private fun formatDate(millis: Long): String {
    val fmt = SimpleDateFormat("yyyy-MM-dd", Locale.US)
    fmt.timeZone = TimeZone.getTimeZone("UTC")
    return fmt.format(Date(millis))
}

private fun toIso(dateMillis: Long, hour: Int, minute: Int): String {
    val cal = Calendar.getInstance(TimeZone.getTimeZone("UTC"))
    cal.timeInMillis = dateMillis
    cal.set(Calendar.HOUR_OF_DAY, hour)
    cal.set(Calendar.MINUTE, minute)
    cal.set(Calendar.SECOND, 0)
    cal.set(Calendar.MILLISECOND, 0)
    val fmt = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.US)
    fmt.timeZone = TimeZone.getTimeZone("UTC")
    return fmt.format(cal.time)
}
