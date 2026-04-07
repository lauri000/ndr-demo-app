package social.innode.ndr.demo.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable

private val LightColors =
    lightColorScheme(
        primary = Sky500,
        secondary = Slate700,
        surface = Sand50,
        background = Sand50,
    )

private val DarkColors =
    darkColorScheme(
        primary = Sky200,
        secondary = Sand50,
        surface = Slate900,
        background = Slate900,
    )

@Composable
fun NdrDemoTheme(
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkColors else LightColors,
        content = content,
    )
}
