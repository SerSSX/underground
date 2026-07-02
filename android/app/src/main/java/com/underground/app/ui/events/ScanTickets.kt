package com.underground.app.ui.events

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import com.underground.app.UndergroundApp
import com.underground.app.data.ValidateReq
import com.underground.app.data.ValidateResp
import kotlinx.coroutines.launch
import retrofit2.HttpException

private data class ScanResultUi(val ok: Boolean, val title: String, val message: String)

private fun scanOptions() = ScanOptions().apply {
    setDesiredBarcodeFormats(ScanOptions.QR_CODE)
    setPrompt("Наведіть на QR квитка · назад — щоб вийти")
    setBeepEnabled(true)
    // Pin portrait via the custom host (default rotates to landscape — awkward at the door).
    setOrientationLocked(true)
    setCaptureActivity(PortraitCaptureActivity::class.java)
}

private fun verdictUi(r: ValidateResp): ScanResultUi = when {
    r.valid -> ScanResultUi(true, "✓ Дійсний", r.eventTitle)
    r.reason == "already_used" -> ScanResultUi(false, "✗ Уже використаний", r.eventTitle)
    r.reason == "wrong_event" -> ScanResultUi(false, "✗ Інша подія", "Цей квиток не на цю подію")
    r.reason == "returned" -> ScanResultUi(false, "✗ Повернений", r.eventTitle)
    r.reason == "not_found" -> ScanResultUi(false, "✗ Невідомий квиток", "QR не знайдено")
    else -> ScanResultUi(false, "✗ Недійсний", r.eventTitle)
}

/**
 * Camera-scan core shared by the organizer (non-scoped `validate`) and delegated door staff
 * (event-scoped `scan`). [validate] performs the network call; [trigger] renders the UI that
 * starts a scan when its lambda is invoked. Permission + result dialog are handled here.
 */
@Composable
private fun TicketScanner(
    validate: suspend (String) -> ValidateResp,
    trigger: @Composable (onClick: () -> Unit) -> Unit,
) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var result by remember { mutableStateOf<ScanResultUi?>(null) }

    val scanLauncher = rememberLauncherForActivityResult(ScanContract()) { res ->
        val qr = res.contents ?: return@rememberLauncherForActivityResult
        scope.launch {
            result = try {
                verdictUi(validate(qr))
            } catch (e: HttpException) {
                when (e.code()) {
                    403 -> ScanResultUi(false, "✗ Немає доступу", "Ви не контролер цієї події")
                    404 -> ScanResultUi(false, "✗ Невідомий квиток", "QR не знайдено")
                    401 -> ScanResultUi(false, "Потрібен вхід", "Увійдіть у застосунок")
                    else -> ScanResultUi(false, "Помилка", "Код ${e.code()}")
                }
            } catch (e: Exception) {
                ScanResultUi(false, "Помилка мережі", e.message ?: "")
            }
        }
    }

    val permLauncher = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
        if (granted) scanLauncher.launch(scanOptions())
    }

    fun startScan() {
        if (ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) {
            scanLauncher.launch(scanOptions())
        } else {
            permLauncher.launch(Manifest.permission.CAMERA)
        }
    }

    trigger { startScan() }

    result?.let { r ->
        AlertDialog(
            onDismissRequest = { result = null },
            confirmButton = { TextButton(onClick = { result = null }) { Text("OK") } },
            title = { Text(r.title, color = if (r.ok) Color(0xFF2FAE5A) else Color(0xFFE2553A)) },
            text = { Text(r.message) },
        )
    }
}

/** Organizer-facing: scan a ticket QR at the door for any of the current user's own events. */
@Composable
fun ScanTicketsButton(modifier: Modifier = Modifier) {
    val api = UndergroundApp.container.eventApi
    TicketScanner(validate = { api.validate(ValidateReq(it)) }) { onClick ->
        IconButton(onClick = onClick, modifier = modifier) {
            Icon(Icons.Filled.QrCodeScanner, contentDescription = "Сканувати квиток")
        }
    }
}

/** Delegated door staff: scan tickets scoped to a single [eventId] via the event-scoped endpoint. */
@Composable
fun EventScanButton(eventId: String, modifier: Modifier = Modifier) {
    val api = UndergroundApp.container.eventApi
    TicketScanner(validate = { api.scanEvent(eventId, ValidateReq(it)) }) { onClick ->
        Button(onClick = onClick, modifier = modifier) {
            Icon(Icons.Filled.QrCodeScanner, contentDescription = null)
            Text("  Сканувати квитки")
        }
    }
}
