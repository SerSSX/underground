package com.underground.app.ui.auth

import android.widget.Toast
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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.underground.app.ui.components.BrandHeader

@Composable
fun AuthScreen(vm: AuthViewModel) {
    var isRegister by remember { mutableStateOf(false) }
    var email by remember { mutableStateOf("") }
    var password by remember { mutableStateOf("") }
    var name by remember { mutableStateOf("") }
    var showForgot by remember { mutableStateOf(false) }
    var forgotEmail by remember { mutableStateOf("") }
    val busy by vm.busy.collectAsState()
    val error by vm.error.collectAsState()
    val context = LocalContext.current

    val canSubmit = !busy && email.isNotBlank() && password.isNotBlank() && (!isRegister || name.isNotBlank())

    Column(
        modifier = Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(vertical = 24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Spacer(Modifier.height(64.dp))
        BrandHeader()
        Spacer(Modifier.height(24.dp))

        if (isRegister) {
            OutlinedTextField(
                value = name,
                onValueChange = { name = it },
                label = { Text("Ім'я") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
            )
            Spacer(Modifier.height(8.dp))
        }
        OutlinedTextField(
            value = email,
            onValueChange = { email = it },
            label = { Text("Email") },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
        )
        Spacer(Modifier.height(8.dp))
        OutlinedTextField(
            value = password,
            onValueChange = { password = it },
            label = { Text("Пароль") },
            singleLine = true,
            visualTransformation = PasswordVisualTransformation(),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
        )

        error?.let {
            Spacer(Modifier.height(8.dp))
            Text(it, color = MaterialTheme.colorScheme.error)
        }

        Spacer(Modifier.height(16.dp))
        Button(
            onClick = { if (isRegister) vm.register(email, password, name) else vm.login(email, password) },
            enabled = canSubmit,
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
        ) {
            if (busy) {
                CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
            } else {
                Text(if (isRegister) "Зареєструватися" else "Увійти")
            }
        }

        TextButton(onClick = { isRegister = !isRegister }) {
            Text(if (isRegister) "Уже є акаунт? Увійти" else "Немає акаунта? Зареєструватися")
        }
        if (!isRegister) {
            TextButton(onClick = { forgotEmail = email; showForgot = true }) {
                Text("Забули пароль?")
            }
        }
        OutlinedButton(
            onClick = { Toast.makeText(context, "Вхід через Google — скоро", Toast.LENGTH_SHORT).show() },
            modifier = Modifier.fillMaxWidth().padding(horizontal = 24.dp),
        ) {
            Text("Continue with Google")
        }
    }

    if (showForgot) {
        AlertDialog(
            onDismissRequest = { showForgot = false },
            title = { Text("Відновлення паролю") },
            text = {
                OutlinedTextField(
                    value = forgotEmail,
                    onValueChange = { forgotEmail = it },
                    label = { Text("Email") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email),
                )
            },
            confirmButton = {
                TextButton(
                    enabled = forgotEmail.isNotBlank(),
                    onClick = {
                        showForgot = false
                        vm.forgotPassword(forgotEmail) { msg -> Toast.makeText(context, msg, Toast.LENGTH_LONG).show() }
                    },
                ) { Text("Надіслати посилання") }
            },
            dismissButton = { TextButton(onClick = { showForgot = false }) { Text("Скасувати") } },
        )
    }
}
