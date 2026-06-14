package com.underground.app.ui.profile

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.Send
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Tune
import androidx.compose.material3.IconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.text.style.TextAlign
import coil.compose.AsyncImage
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.underground.app.data.UserDto
import com.underground.app.ui.components.PostCard
import com.underground.app.ui.theme.Accent
import com.underground.app.ui.theme.Muted
import com.underground.app.ui.wall.WallViewModel

/** Own profile: header with live counters, post composer, own wall, messages entry. */
@Composable
fun ProfileScreen(
    user: UserDto,
    profileVm: PublicProfileViewModel,
    wallVm: WallViewModel,
    onOpenSettings: () -> Unit,
    onOpenInterests: () -> Unit,
    onOpenEvent: (String) -> Unit,
    onOpenComments: (String) -> Unit,
) {
    LaunchedEffect(user.id) {
        profileVm.load(user.id)
        wallVm.load(user.id)
    }
    val state by profileVm.state.collectAsState()
    val posts by wallVm.posts.collectAsState()
    val busy by wallVm.busy.collectAsState()
    var composer by remember { mutableStateOf("") }

    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                    IconButton(onClick = onOpenSettings) {
                        Icon(Icons.Filled.Settings, contentDescription = "Налаштування", tint = Muted)
                    }
                }
                Box(
                    modifier = Modifier.size(88.dp).clip(CircleShape).background(Accent),
                    contentAlignment = Alignment.Center,
                ) {
                    if (user.avatarUrl != null) {
                        AsyncImage(
                            model = user.avatarUrl,
                            contentDescription = "Аватар",
                            contentScale = ContentScale.Crop,
                            modifier = Modifier.fillMaxSize(),
                        )
                    } else {
                        Text(
                            user.name.take(1).uppercase(),
                            style = MaterialTheme.typography.headlineMedium,
                            color = Color.White,
                            fontWeight = FontWeight.Bold,
                        )
                    }
                }
                Spacer(Modifier.height(10.dp))
                Text(user.name, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
                Text(user.email, color = Muted)
                user.bio?.takeIf { it.isNotBlank() }?.let {
                    Text(
                        it,
                        color = Muted,
                        textAlign = TextAlign.Center,
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(top = 4.dp, start = 16.dp, end = 16.dp),
                    )
                }
                Spacer(Modifier.height(6.dp))
                Surface(color = Accent, shape = RoundedCornerShape(50)) {
                    Text(
                        roleLabel(user.role, user.isOrganizer),
                        style = MaterialTheme.typography.labelMedium,
                        color = Color.White,
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
                    )
                }
                Spacer(Modifier.height(16.dp))
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly) {
                    Stat("${state.profile?.followersCount ?: 0}", "Підписники")
                    Stat("${state.profile?.followingCount ?: 0}", "Підписки")
                    Stat("${state.profile?.eventsCount ?: 0}", "Події")
                    Stat("${posts.size}", "Пости")
                }
                Spacer(Modifier.height(12.dp))
                OutlinedButton(onClick = onOpenInterests, modifier = Modifier.fillMaxWidth()) {
                    Icon(Icons.Filled.Tune, contentDescription = null)
                    Spacer(Modifier.width(6.dp))
                    Text("Мої інтереси")
                }
            }
        }

        item {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                OutlinedTextField(
                    value = composer,
                    onValueChange = { composer = it },
                    placeholder = { Text("Поділіться чимось…") },
                    modifier = Modifier.weight(1f),
                )
                IconButton(
                    onClick = { wallVm.createPost(composer) { composer = "" } },
                    enabled = composer.isNotBlank() && !busy,
                ) {
                    Icon(Icons.AutoMirrored.Filled.Send, contentDescription = "Опублікувати", tint = Accent)
                }
            }
        }

        if (posts.isEmpty()) {
            item { Text("Стіна порожня. Сходіть на подію і залиште відгук!", color = Muted) }
        }
        items(posts, key = { it.id }) { post ->
            PostCard(
                post = post,
                isMine = true,
                onToggleLike = { wallVm.toggleLike(post) },
                onOpenComments = { onOpenComments(post.id) },
                onOpenEvent = onOpenEvent,
                onOpenAuthor = {},
                onDelete = { wallVm.deletePost(post) },
            )
        }
    }
}

@Composable
private fun Stat(value: String, label: String) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(value, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
        Text(label, style = MaterialTheme.typography.labelMedium, color = Muted)
    }
}

private fun roleLabel(role: String, isOrganizer: Boolean): String = when {
    role.equals("ADMIN", ignoreCase = true) -> "Адміністратор"
    isOrganizer -> "Організатор"
    else -> "Користувач"
}
