package com.underground.app.ui.gift

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.underground.app.data.GiftPreviewDto
import com.underground.app.ui.components.BrandHeader

/**
 * Deep-link claim landing (TASK-114, WF-GiftClaim). Shows the gift with NO account
 * (browse-before-auth); claiming signs the recipient in with a passwordless email code
 * (account created on first use) and transfers the ticket. `onDone` lets the host refresh
 * into the signed-in app.
 */
@Composable
fun ClaimGiftScreen(token: String, onDone: () -> Unit, vm: ClaimGiftViewModel = viewModel()) {
    val state by vm.state.collectAsState()
    LaunchedEffect(token) { vm.load(token) }

    when (val s = state) {
        ClaimState.Loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
            CircularProgressIndicator()
        }
        ClaimState.Gone -> Centered {
            BrandHeader()
            Spacer(Modifier.height(24.dp))
            Text("Подарунок недоступний", style = MaterialTheme.typography.titleLarge)
            Spacer(Modifier.height(8.dp))
            Text(
                "Це посилання вже використали або воно протерміноване.",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(16.dp))
            Button(onClick = onDone, modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp)) {
                Text("До подій")
            }
        }
        ClaimState.Done -> LaunchedEffect(Unit) { onDone() }
        is ClaimState.Ready -> ReadyView(s.preview, vm, onDone)
    }
}

@Composable
private fun ReadyView(preview: GiftPreviewDto, vm: ClaimGiftViewModel, onDone: () -> Unit) {
    val busy by vm.busy.collectAsState()
    val error by vm.error.collectAsState()
    val codeSent by vm.codeSent.collectAsState()
    var email by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }

    Centered {
        BrandHeader()
        Spacer(Modifier.height(16.dp))
        Text("🎁", style = MaterialTheme.typography.displaySmall)
        Text(
            "${preview.fromName} дарує тобі квиток",
            style = MaterialTheme.typography.titleLarge,
            fontWeight = FontWeight.Bold,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(horizontal = 24.dp, vertical = 8.dp),
        )
        Text(preview.eventTitle, style = MaterialTheme.typography.titleMedium)
        Text(
            listOfNotNull(preview.eventStartsAt?.take(10), preview.eventCity).joinToString(" · "),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Spacer(Modifier.height(16.dp))

        error?.let {
            Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.padding(horizontal = 24.dp))
            Spacer(Modifier.height(8.dp))
        }

        val wide = Modifier.fillMaxWidth().padding(horizontal = 24.dp)
        when {
            vm.loggedIn -> ActionButton("Забрати квиток", busy, wide) { vm.claim() }
            !codeSent -> {
                OutlinedTextField(
                    value = email,
                    onValueChange = { email = it },
                    label = { Text("Email") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                    modifier = wide,
                )
                Spacer(Modifier.height(8.dp))
                ActionButton("Отримати код на email", busy, wide, enabled = email.isNotBlank()) { vm.sendCode(email) }
                Text(
                    "Акаунт створиться автоматично.",
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(top = 6.dp),
                )
            }
            else -> {
                OutlinedTextField(
                    value = code,
                    onValueChange = { code = it },
                    label = { Text("Код з email") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                    modifier = wide,
                )
                Spacer(Modifier.height(8.dp))
                ActionButton("Забрати квиток", busy, wide, enabled = code.isNotBlank()) {
                    vm.verifyAndClaim(email, code)
                }
            }
        }
        Spacer(Modifier.height(14.dp))
        Text(
            "Безпечна передача · escrow",
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            style = MaterialTheme.typography.bodySmall,
        )
        TextButton(onClick = onDone) { Text("Пізніше") }
    }
}

@Composable
private fun ActionButton(label: String, busy: Boolean, modifier: Modifier, enabled: Boolean = true, onClick: () -> Unit) {
    Button(onClick = onClick, enabled = enabled && !busy, modifier = modifier) {
        if (busy) CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp) else Text(label)
    }
}

@Composable
private fun Centered(content: @Composable () -> Unit) {
    Column(
        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(vertical = 48.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) { content() }
}
