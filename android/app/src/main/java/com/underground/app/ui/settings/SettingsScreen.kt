package com.underground.app.ui.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Logout
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import android.content.Intent
import android.net.Uri
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import coil.compose.AsyncImage
import com.underground.app.data.UserDto
import com.underground.app.ui.theme.Accent
import com.underground.app.ui.theme.Muted

/** Account settings: avatar, name, bio, organizer mode, logout (hidden away from the profile). */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    vm: SettingsViewModel,
    user: UserDto,
    onSaved: (UserDto) -> Unit,
    onLogout: () -> Unit,
    onBack: () -> Unit,
) {
    var name by remember { mutableStateOf(user.name) }
    var bio by remember { mutableStateOf(user.bio.orEmpty()) }
    var isOrganizer by remember { mutableStateOf(user.isOrganizer) }
    var avatarUrl by remember { mutableStateOf(user.avatarUrl) }
    var dob by remember { mutableStateOf("") }
    var payoutCountry by remember { mutableStateOf("") }
    var showDelete by remember { mutableStateOf(false) }
    var deletePw by remember { mutableStateOf("") }

    val busy by vm.busy.collectAsState()
    val uploading by vm.uploading.collectAsState()
    val error by vm.error.collectAsState()
    val message by vm.message.collectAsState()
    val payouts by vm.payouts.collectAsState()
    val context = LocalContext.current
    LaunchedEffect(Unit) { vm.loadPayouts() }

    val pickAvatar = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        val bytes = context.contentResolver.openInputStream(uri)?.use { it.readBytes() } ?: return@rememberLauncherForActivityResult
        val mime = context.contentResolver.getType(uri) ?: "image/jpeg"
        vm.uploadAvatar(bytes, mime) { url -> avatarUrl = url }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Налаштування") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
                    }
                },
            )
        },
    ) { inner ->
        Column(
            modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(inner).padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                Box(
                    modifier = Modifier.size(96.dp).clip(CircleShape).background(Accent),
                    contentAlignment = Alignment.Center,
                ) {
                    if (avatarUrl != null) {
                        AsyncImage(
                            model = avatarUrl,
                            contentDescription = "Аватар",
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Text(
                            name.take(1).uppercase(),
                            style = MaterialTheme.typography.headlineMedium,
                            color = Color.White,
                            fontWeight = FontWeight.Bold,
                        )
                    }
                }
            }
            OutlinedButton(
                onClick = { pickAvatar.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) },
                enabled = !uploading,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(if (uploading) "Завантаження…" else "Змінити аватар")
            }

            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("Ім'я") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = bio,
                onValueChange = { bio = it },
                label = { Text("Про себе") },
                minLines = 2,
                modifier = Modifier.fillMaxWidth(),
            )

            Row(
                Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Column(Modifier.weight(1f).padding(end = 8.dp)) {
                    Text("Режим організатора")
                    Text(
                        "Показує бейдж «Організатор». Створювати події може будь-хто.",
                        style = MaterialTheme.typography.bodySmall,
                        color = Muted,
                    )
                }
                Switch(checked = isOrganizer, onCheckedChange = { isOrganizer = it })
            }

            error?.let { Text(it, color = MaterialTheme.colorScheme.error) }

            Button(
                onClick = {
                    vm.save(
                        name = name,
                        bio = bio,
                        avatarUrl = avatarUrl,
                        isOrganizer = isOrganizer,
                        onSaved = onSaved,
                    )
                },
                enabled = name.isNotBlank() && !busy && !uploading,
                modifier = Modifier.fillMaxWidth(),
            ) {
                if (busy) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                } else {
                    Text("Зберегти")
                }
            }

            message?.let { Text(it, color = Accent) }

            HorizontalDivider()
            Text("Дата народження", fontWeight = FontWeight.SemiBold)
            Text("Потрібна для подій із віковим обмеженням.", style = MaterialTheme.typography.bodySmall, color = Muted)
            val dobValid = SettingsViewModel.isValidDob(dob)
            OutlinedTextField(
                value = dob,
                onValueChange = { dob = it },
                label = { Text("РРРР-ММ-ДД") },
                singleLine = true,
                isError = dob.isNotBlank() && !dobValid,
                modifier = Modifier.fillMaxWidth(),
            )
            if (dob.isNotBlank() && !dobValid) {
                Text("Невірна дата. Формат: РРРР-ММ-ДД", style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
            }
            OutlinedButton(onClick = { vm.saveDob(dob) }, enabled = dobValid && !busy, modifier = Modifier.fillMaxWidth()) {
                Text("Зберегти дату народження")
            }

            HorizontalDivider()
            Text("Виплати організатора", fontWeight = FontWeight.SemiBold)
            val p = payouts
            if (p == null || !p.connected) {
                Text(
                    "Щоб продавати платні квитки, підключіть виплати. Свої дані й банківський рахунок " +
                        "ви вводите на сторінці платіжної системи — застосунок їх не бачить.",
                    style = MaterialTheme.typography.bodySmall,
                    color = Muted,
                )
                OutlinedTextField(
                    value = payoutCountry,
                    onValueChange = { payoutCountry = it.uppercase().take(2) },
                    label = { Text("Країна (2 літери, напр. PL)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            } else {
                Text(
                    "Приймання оплат: " + (if (p.chargesEnabled) "увімкнено" else "перевіряється") +
                        " · Виплати: " + (if (p.payoutsEnabled) "увімкнено" else "перевіряється"),
                    style = MaterialTheme.typography.bodySmall,
                    color = Muted,
                )
                if (p.requirementsDue.isNotEmpty()) {
                    Text(
                        "Платіжна система чекає документи: " + p.requirementsDue.joinToString(", "),
                        style = MaterialTheme.typography.bodySmall,
                        color = Muted,
                    )
                }
            }
            OutlinedButton(
                onClick = {
                    // The onboarding URL is single-use and short-lived, so it is opened straight
                    // away in the browser and never kept.
                    vm.connectPayouts(p?.country ?: payoutCountry) { url ->
                        context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                    }
                },
                enabled = !busy && ((p?.connected == true) || payoutCountry.length == 2),
                modifier = Modifier.fillMaxWidth(),
            ) { Text(if (p?.connected == true) "Змінити реквізити" else "Підключити виплати") }

            HorizontalDivider()
            Text("Ваші дані (GDPR)", fontWeight = FontWeight.SemiBold)
            OutlinedButton(
                onClick = {
                    vm.exportData { json ->
                        val share = Intent(Intent.ACTION_SEND).apply {
                            type = "application/json"
                            putExtra(Intent.EXTRA_TEXT, json)
                        }
                        context.startActivity(Intent.createChooser(share, "Експорт даних"))
                    }
                },
                modifier = Modifier.fillMaxWidth(),
            ) { Text("Експортувати мої дані") }
            OutlinedButton(
                onClick = { showDelete = true },
                colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error),
                modifier = Modifier.fillMaxWidth(),
            ) { Text("Видалити акаунт") }

            Spacer(Modifier.height(8.dp))
            OutlinedButton(
                onClick = onLogout,
                colors = ButtonDefaults.outlinedButtonColors(contentColor = MaterialTheme.colorScheme.error),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Icon(Icons.AutoMirrored.Filled.Logout, contentDescription = null)
                Spacer(Modifier.size(6.dp))
                Text("Вийти з акаунта")
            }
        }
    }

    if (showDelete) {
        AlertDialog(
            onDismissRequest = { showDelete = false },
            title = { Text("Видалити акаунт?") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Дію не можна скасувати. Введіть пароль для підтвердження.")
                    OutlinedTextField(
                        value = deletePw,
                        onValueChange = { deletePw = it },
                        label = { Text("Пароль") },
                        singleLine = true,
                        visualTransformation = PasswordVisualTransformation(),
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            },
            confirmButton = {
                TextButton(
                    onClick = { vm.deleteAccount(deletePw) { showDelete = false; onLogout() } },
                    enabled = deletePw.isNotBlank() && !busy,
                ) { Text("Видалити", color = MaterialTheme.colorScheme.error) }
            },
            dismissButton = { TextButton(onClick = { showDelete = false }) { Text("Скасувати") } },
        )
    }
}
