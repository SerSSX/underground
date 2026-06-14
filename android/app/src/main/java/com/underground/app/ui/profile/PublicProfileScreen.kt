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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.underground.app.ui.components.PostCard
import com.underground.app.ui.theme.Accent
import com.underground.app.ui.theme.Muted
import com.underground.app.ui.wall.WallViewModel
import java.util.Locale

/** Someone else's profile: counters, follow/unfollow, wall, events, message button. */
@Composable
fun PublicProfileScreen(
    vm: PublicProfileViewModel,
    wallVm: WallViewModel,
    userId: String,
    currentUserId: String,
    onOpenEvent: (String) -> Unit,
    onMessage: (peerId: String, peerName: String) -> Unit,
    onOpenComments: (String) -> Unit,
    onBack: () -> Unit,
) {
    LaunchedEffect(userId) {
        vm.load(userId)
        wallVm.load(userId)
    }
    val state by vm.state.collectAsState()
    val posts by wallVm.posts.collectAsState()

    Column(Modifier.fillMaxSize()) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(4.dp)) {
            IconButton(onClick = onBack) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
            }
            Text("Профіль", style = MaterialTheme.typography.titleMedium)
        }

        when {
            state.loading -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                CircularProgressIndicator()
            }
            state.error != null -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text(state.error.orEmpty(), color = MaterialTheme.colorScheme.error)
            }
            else -> state.profile?.let { profile ->
                LazyColumn(
                    contentPadding = PaddingValues(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                    modifier = Modifier.fillMaxSize(),
                ) {
                    item {
                        Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
                            Box(
                                modifier = Modifier.size(88.dp).clip(CircleShape).background(Accent),
                                contentAlignment = Alignment.Center,
                            ) {
                                Text(
                                    profile.name.take(1).uppercase(),
                                    style = MaterialTheme.typography.headlineMedium,
                                    color = Color.White,
                                    fontWeight = FontWeight.Bold,
                                )
                            }
                            Spacer(Modifier.height(10.dp))
                            Text(profile.name, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
                            profile.bio?.let {
                                Text(it, color = Muted, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 4.dp))
                            }
                            profile.ratingAvg?.let { avg ->
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    modifier = Modifier.padding(top = 6.dp),
                                ) {
                                    Icon(Icons.Filled.Star, contentDescription = null, tint = Accent)
                                    Text(
                                        String.format(Locale.US, " %.1f · відгуків: %d", avg, profile.ratingCount),
                                        color = Muted,
                                        style = MaterialTheme.typography.bodyMedium,
                                    )
                                }
                            }
                            Spacer(Modifier.height(16.dp))
                            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceEvenly) {
                                Stat("${profile.followersCount}", "Підписники")
                                Stat("${profile.followingCount}", "Підписки")
                                Stat("${profile.eventsCount}", "Події")
                            }
                            if (profile.id != currentUserId) {
                                Spacer(Modifier.height(16.dp))
                                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                    if (profile.isFollowing) {
                                        OutlinedButton(onClick = vm::toggleFollow, modifier = Modifier.weight(1f)) {
                                            Text("Ви підписані")
                                        }
                                    } else {
                                        Button(onClick = vm::toggleFollow, modifier = Modifier.weight(1f)) {
                                            Text("Підписатись")
                                        }
                                    }
                                    OutlinedButton(
                                        onClick = { onMessage(profile.id, profile.name) },
                                        modifier = Modifier.weight(1f),
                                    ) {
                                        Text("Написати")
                                    }
                                }
                            }
                        }
                    }
                    if (posts.isNotEmpty()) {
                        item {
                            Text("Стіна", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                        }
                        items(posts, key = { "post-" + it.id }) { post ->
                            PostCard(
                                post = post,
                                isMine = false,
                                onToggleLike = { wallVm.toggleLike(post) },
                                onOpenComments = { onOpenComments(post.id) },
                                onOpenEvent = onOpenEvent,
                                onOpenAuthor = {},
                            )
                        }
                    }
                    if (state.events.isNotEmpty()) {
                        item {
                            Text("Події", style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
                        }
                        items(state.events, key = { it.id }) { event ->
                            com.underground.app.ui.components.EventCard(event) { onOpenEvent(event.id) }
                        }
                    }
                }
            }
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
