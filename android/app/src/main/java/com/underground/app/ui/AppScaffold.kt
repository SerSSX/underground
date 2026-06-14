package com.underground.app.ui

import android.net.Uri
import android.widget.Toast
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Mail
import androidx.compose.material.icons.filled.Person
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.ViewDay
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.Icon
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.underground.app.data.UserDto
import com.underground.app.ui.afisha.AfishaScreen
import com.underground.app.ui.afisha.AfishaViewModel
import com.underground.app.ui.alerts.AlertsViewModel
import com.underground.app.ui.chat.ChatScreen
import com.underground.app.ui.chat.ChatViewModel
import com.underground.app.ui.chat.ConversationsViewModel
import com.underground.app.ui.create.CreateEventScreen
import com.underground.app.ui.detail.EventDetailScreen
import com.underground.app.ui.detail.EventExtrasViewModel
import com.underground.app.ui.detail.ReviewsViewModel
import com.underground.app.ui.events.EditEventScreen
import com.underground.app.ui.events.MyEventsViewModel
import com.underground.app.ui.feed.FeedScreen
import com.underground.app.ui.feed.FeedViewModel
import com.underground.app.ui.inbox.InboxScreen
import com.underground.app.ui.inbox.UnreadViewModel
import com.underground.app.ui.mine.MineScreen
import com.underground.app.ui.onboarding.InterestsScreen
import com.underground.app.ui.onboarding.InterestsViewModel
import com.underground.app.ui.profile.ProfileScreen
import com.underground.app.ui.profile.PublicProfileScreen
import com.underground.app.ui.profile.PublicProfileViewModel
import com.underground.app.ui.settings.SettingsScreen
import com.underground.app.ui.settings.SettingsViewModel
import com.underground.app.ui.tickets.ETicketScreen
import com.underground.app.ui.tickets.TestPayScreen
import com.underground.app.ui.tickets.TicketsViewModel
import com.underground.app.ui.wall.CommentsScreen
import com.underground.app.ui.wall.CommentsViewModel
import com.underground.app.ui.wall.WallViewModel

private enum class Tab(val route: String, val label: String, val icon: ImageVector) {
    Afisha("afisha", "Афіша", Icons.Filled.Search),
    Feed("feed", "Стрічка", Icons.Filled.ViewDay),
    Mine("mine", "Мої", Icons.Filled.Star),
    Inbox("inbox", "Вхідні", Icons.Filled.Mail),
    Profile("profile", "Профіль", Icons.Filled.Person),
}

/** Routes that take the full screen: no bottom bar clutter, no FAB. */
// "interests" keeps the bottom bar (its editor has no own top bar — never a dead-end).
private val FULLSCREEN_PREFIXES =
    listOf("create", "detail", "ticket", "pay", "user", "chat", "comments", "editEvent", "settings")

@Composable
fun AppScaffold(user: UserDto, onLogout: () -> Unit, onUserUpdated: (UserDto) -> Unit) {
    val nav = rememberNavController()
    val context = LocalContext.current
    val afishaVm: AfishaViewModel = viewModel()
    val myEventsVm: MyEventsViewModel = viewModel()
    val ticketsVm: TicketsViewModel = viewModel()
    val unreadVm: UnreadViewModel = viewModel()
    val unread by unreadVm.unread.collectAsState()

    val backStack by nav.currentBackStackEntryAsState()
    val current = backStack?.destination
    val route = current?.route
    // Refresh the inbox badge on every navigation — cheap counters endpoint.
    LaunchedEffect(backStack) { unreadVm.refresh() }
    val fullscreen = FULLSCREEN_PREFIXES.any { route?.startsWith(it) == true }

    val openChat = { peerId: String, peerName: String ->
        nav.navigate("chat/$peerId/${Uri.encode(peerName)}")
    }

    Scaffold(
        bottomBar = {
            if (!fullscreen) {
                NavigationBar {
                    Tab.entries.forEach { tab ->
                        NavigationBarItem(
                            selected = current?.hierarchy?.any { it.route == tab.route } == true,
                            onClick = {
                                nav.navigate(tab.route) {
                                    popUpTo(nav.graph.findStartDestination().id) { saveState = true }
                                    launchSingleTop = true
                                    restoreState = true
                                }
                            },
                            icon = {
                                val badgeCount = if (tab == Tab.Inbox) unread.notifications + unread.messages else 0
                                if (badgeCount > 0) {
                                    BadgedBox(badge = { Badge { Text("$badgeCount") } }) {
                                        Icon(tab.icon, contentDescription = tab.label)
                                    }
                                } else {
                                    Icon(tab.icon, contentDescription = tab.label)
                                }
                            },
                            label = { Text(tab.label, maxLines = 1) },
                        )
                    }
                }
            }
        },
    ) { inner ->
        NavHost(
            navController = nav,
            startDestination = Tab.Afisha.route,
            modifier = Modifier.padding(inner),
        ) {
            composable(Tab.Afisha.route) {
                AfishaScreen(afishaVm) { id -> nav.navigate("detail/$id") }
            }
            composable(Tab.Feed.route) { entry ->
                val feedVm: FeedViewModel = viewModel(entry)
                FeedScreen(
                    vm = feedVm,
                    currentUserId = user.id,
                    onOpenEvent = { id -> nav.navigate("detail/$id") },
                    onOpenAuthor = { id -> nav.navigate("user/$id") },
                    onOpenComments = { postId -> nav.navigate("comments/$postId") },
                )
            }
            composable("detail/{eventId}") { entry ->
                val id = entry.arguments?.getString("eventId").orEmpty()
                val reviewsVm: ReviewsViewModel = viewModel(entry)
                val extrasVm: EventExtrasViewModel = viewModel(entry)
                LaunchedEffect(id) { extrasVm.load(id) }
                val savedIds by afishaVm.savedIds.collectAsState()
                val social by extrasVm.social.collectAsState()
                val similar by extrasVm.similar.collectAsState()
                val fetched by extrasVm.event.collectAsState()
                EventDetailScreen(
                    event = afishaVm.find(id) ?: myEventsVm.find(id) ?: fetched,
                    reviewsVm = reviewsVm,
                    social = social,
                    similar = similar,
                    saved = id in savedIds,
                    onToggleSave = { afishaVm.toggleSave(id) },
                    onOpenEvent = { eid -> nav.navigate("detail/$eid") },
                    onReserve = { eventId ->
                        ticketsVm.reserve(
                            eventId,
                            onSuccess = { ticket -> nav.navigate("ticket/${ticket.id}") },
                            onError = { msg -> Toast.makeText(context, msg, Toast.LENGTH_SHORT).show() },
                        )
                    },
                    onBuy = { eventId ->
                        ticketsVm.checkout(
                            eventId,
                            onReady = { c -> nav.navigate("pay/${c.orderId}/${c.amount}/${c.currency}") },
                            onError = { msg -> Toast.makeText(context, msg, Toast.LENGTH_SHORT).show() },
                        )
                    },
                    onOpenOrganizer = { organizerId -> nav.navigate("user/$organizerId") },
                    onMessageOrganizer = { organizerId -> openChat(organizerId, "Організатор") },
                    onBack = { nav.popBackStack() },
                )
            }
            composable("user/{userId}") { entry ->
                val id = entry.arguments?.getString("userId").orEmpty()
                val profileVm: PublicProfileViewModel = viewModel(entry)
                val wallVm: WallViewModel = viewModel(entry)
                PublicProfileScreen(
                    vm = profileVm,
                    wallVm = wallVm,
                    userId = id,
                    currentUserId = user.id,
                    onOpenEvent = { eventId -> nav.navigate("detail/$eventId") },
                    onMessage = openChat,
                    onOpenComments = { postId -> nav.navigate("comments/$postId") },
                    onBack = { nav.popBackStack() },
                )
            }
            composable("chat/{peerId}/{peerName}") { entry ->
                val peerId = entry.arguments?.getString("peerId").orEmpty()
                val peerName = Uri.decode(entry.arguments?.getString("peerName").orEmpty())
                val chatVm: ChatViewModel = viewModel(entry)
                ChatScreen(
                    vm = chatVm,
                    peerId = peerId,
                    peerName = peerName,
                    currentUserId = user.id,
                    onBack = { nav.popBackStack() },
                )
            }
            composable("interests") { entry ->
                val interestsVm: InterestsViewModel = viewModel(entry)
                InterestsScreen(interestsVm) { nav.popBackStack() }
            }
            composable("settings") { entry ->
                val settingsVm: SettingsViewModel = viewModel(entry)
                SettingsScreen(
                    vm = settingsVm,
                    user = user,
                    onSaved = { updated ->
                        onUserUpdated(updated)
                        nav.popBackStack()
                    },
                    onLogout = onLogout,
                    onBack = { nav.popBackStack() },
                )
            }
            composable("comments/{postId}") { entry ->
                val postId = entry.arguments?.getString("postId").orEmpty()
                val commentsVm: CommentsViewModel = viewModel(entry)
                CommentsScreen(commentsVm, postId) { nav.popBackStack() }
            }
            composable("ticket/{ticketId}") { entry ->
                val id = entry.arguments?.getString("ticketId").orEmpty()
                ETicketScreen(ticketsVm, ticketsVm.find(id)) { nav.popBackStack() }
            }
            composable("pay/{orderId}/{amount}/{currency}") { entry ->
                val orderId = entry.arguments?.getString("orderId").orEmpty()
                val amount = entry.arguments?.getString("amount").orEmpty()
                val currency = entry.arguments?.getString("currency").orEmpty()
                TestPayScreen(
                    vm = ticketsVm,
                    orderId = orderId,
                    amount = amount,
                    currency = currency,
                    onPaid = { ticket ->
                        nav.navigate("ticket/${ticket.id}") {
                            popUpTo("pay/{orderId}/{amount}/{currency}") { inclusive = true }
                        }
                    },
                    onBack = { nav.popBackStack() },
                )
            }
            composable("create") {
                CreateEventScreen(
                    onDone = {
                        nav.popBackStack()
                        afishaVm.load()
                        myEventsVm.load()
                    },
                    onBack = { nav.popBackStack() },
                )
            }
            composable("editEvent/{eventId}") { entry ->
                val id = entry.arguments?.getString("eventId").orEmpty()
                EditEventScreen(
                    vm = myEventsVm,
                    event = myEventsVm.find(id),
                    onDone = {
                        nav.popBackStack()
                        afishaVm.load()
                    },
                    onBack = { nav.popBackStack() },
                )
            }
            composable(Tab.Mine.route) {
                MineScreen(
                    ticketsVm = ticketsVm,
                    myEventsVm = myEventsVm,
                    onCreate = { nav.navigate("create") },
                    onOpenEvent = { id -> nav.navigate("detail/$id") },
                    onOpenTicket = { id -> nav.navigate("ticket/$id") },
                    onManageEvent = { id -> nav.navigate("editEvent/$id") },
                )
            }
            composable(Tab.Inbox.route) { entry ->
                val alertsVm: AlertsViewModel = viewModel(entry)
                val convVm: ConversationsViewModel = viewModel(entry)
                InboxScreen(
                    alertsVm = alertsVm,
                    conversationsVm = convVm,
                    unread = unread,
                    onOpenEvent = { id -> nav.navigate("detail/$id") },
                    onOpenChat = openChat,
                )
            }
            composable(Tab.Profile.route) { entry ->
                val profileVm: PublicProfileViewModel = viewModel(entry)
                val wallVm: WallViewModel = viewModel(entry)
                ProfileScreen(
                    user = user,
                    profileVm = profileVm,
                    wallVm = wallVm,
                    onOpenSettings = { nav.navigate("settings") },
                    onOpenInterests = { nav.navigate("interests") },
                    onOpenEvent = { id -> nav.navigate("detail/$id") },
                    onOpenComments = { postId -> nav.navigate("comments/$postId") },
                )
            }
        }
    }
}
