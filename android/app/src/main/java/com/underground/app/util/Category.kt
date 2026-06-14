package com.underground.app.util

import androidx.compose.ui.graphics.Color

private val MusicRed = Color(0xFFE23A45)
private val Purple = Color(0xFF6C4BF6)
private val Violet = Color(0xFFB23AE2)
private val Pink = Color(0xFFE84B8A)
private val Teal = Color(0xFF2E8B7F)
private val Amber = Color(0xFFF2A33C)
private val Gold = Color(0xFFC0853E)
private val Blue = Color(0xFF2F6BFF)
private val Green = Color(0xFF2FAE5A)
private val Cyan = Color(0xFF2FA9C7)
private val Neutral = Color(0xFF5A5A66)

/** Accent colour per event category (grouped) — poster gradients and chips. */
fun categoryColor(category: String): Color = when (category.lowercase()) {
    "rock", "metal", "concert" -> MusicRed
    "electronic", "dj set", "festival" -> Purple
    "hip-hop" -> Violet
    "pop", "karaoke" -> Pink
    "jazz", "folk", "classical" -> Teal
    "stand-up", "improv", "theatre", "dance", "poetry", "cinema" -> Amber
    "exhibition", "photography", "fashion", "literature", "craft" -> Gold
    "lecture", "conference", "workshop", "course", "networking", "startup", "hackathon", "business" -> Blue
    "sport", "yoga", "hike", "run", "cycling", "extreme" -> Green
    "party", "meetup", "charity", "family", "food & drink", "tasting",
    "board games", "quiz", "market", "gaming", "spiritual", "entertainment" -> Cyan
    else -> Neutral
}
