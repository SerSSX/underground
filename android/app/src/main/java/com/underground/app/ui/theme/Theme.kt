package com.underground.app.ui.theme

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier

private val DarkColors = darkColorScheme(
    primary = Accent,
    onPrimary = OnDark,
    background = Background,
    onBackground = OnDark,
    surface = SurfaceDark,
    onSurface = OnDark,
    surfaceVariant = SurfaceDark,
    onSurfaceVariant = Muted,
)

/** Always-dark theme, regardless of the system setting. A root [Surface] sets the background
 * and, crucially, provides the default content color (onBackground) so bare `Text` isn't the
 * Material default black-on-black on screens rendered outside a Scaffold (auth, gift claim). */
@Composable
fun UndergroundTheme(content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = DarkColors) {
        Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background, content = content)
    }
}
