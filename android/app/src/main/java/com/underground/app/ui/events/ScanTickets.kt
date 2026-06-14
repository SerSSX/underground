package com.underground.app.ui.events

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.AlertDialog
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

/** Organizer-facing: scan a ticket QR at the door and show the validation verdict. */
@Composable
fun ScanTicketsButton(modifier: Modifier = Modifier) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var result by remember { mutableStateOf<ScanResultUi?>(null) }

    val scanLauncher = rememberLauncherForActivityResult(ScanContract()) { res ->
        val qr = res.contents ?: return@rememberLauncherForActivityResult
        scope.launch {
            result = try {
                val r = UndergroundApp.container.eventApi.validate(ValidateReq(qr))
                when {
                    r.valid -> ScanResultUi(true, "✓ Дійсний", r.eventTitle)
                    r.reason == "already_used" -> ScanResultUi(false, "✗ Уже використаний", r.eventTitle)
                    else -> ScanResultUi(false, "✗ Недійсний", r.eventTitle)
                }
            } catch (e: HttpException) {
                when (e.code()) {
                    403 -> ScanResultUi(false, "✗ Не ваша подія", "Цей квиток не на вашу подію")
                    404 -> ScanResultUi(false, "✗ Невідомий квиток", "QR не знайдено")
                    401 -> ScanResultUi(false, "Потрібен вхід", "Увійдіть як організатор")
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

    IconButton(onClick = { startScan() }, modifier = modifier) {
        Icon(Icons.Filled.QrCodeScanner, contentDescription = "Сканувати квиток")
    }

    result?.let { r ->
        AlertDialog(
            onDismissRequest = { result = null },
            confirmButton = { TextButton(onClick = { result = null }) { Text("OK") } },
            title = { Text(r.title, color = if (r.ok) Color(0xFF2FAE5A) else Color(0xFFE2553A)) },
            text = { Text(r.message) },
        )
    }
}
