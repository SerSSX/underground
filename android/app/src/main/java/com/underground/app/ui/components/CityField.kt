package com.underground.app.ui.components

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.foundation.clickable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.underground.app.UndergroundApp
import com.underground.app.data.CityDto
import com.underground.app.ui.theme.Muted

/** City input with gazetteer autocomplete (TASK-017 M2). Picking a known city unlocks geo. */
@Composable
fun CityField(value: String, label: String = "Місто", onPick: (String) -> Unit) {
    var query by remember { mutableStateOf(value) }
    var suggestions by remember { mutableStateOf<List<CityDto>>(emptyList()) }
    var justPicked by remember { mutableStateOf(false) }

    LaunchedEffect(query) {
        if (justPicked || query.length < 2) {
            suggestions = emptyList()
            return@LaunchedEffect
        }
        suggestions = try {
            UndergroundApp.container.userApi.cities(query)
        } catch (e: Exception) {
            emptyList()
        }
    }

    Column(Modifier.fillMaxWidth()) {
        OutlinedTextField(
            value = query,
            onValueChange = {
                query = it
                justPicked = false
                onPick(it) // free text still accepted; geo resolves only for known cities
            },
            label = { Text(label) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        if (suggestions.isNotEmpty()) {
            Card(
                modifier = Modifier.fillMaxWidth(),
                colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surface),
            ) {
                LazyColumn(Modifier.heightIn(max = 200.dp)) {
                    items(suggestions, key = { it.name }) { city ->
                        Text(
                            "${city.name} · ${city.country}",
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    justPicked = true
                                    query = city.name
                                    suggestions = emptyList()
                                    onPick(city.name)
                                }
                                .padding(horizontal = 14.dp, vertical = 12.dp),
                            style = MaterialTheme.typography.bodyMedium,
                            color = Muted,
                        )
                    }
                }
            }
        }
    }
}
