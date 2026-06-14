package com.underground.app.ui.theme

import androidx.compose.ui.graphics.Color

// Mirrors the single source of truth at shared/design-tokens.css (FS-1).
// Keep these in sync with that file — web/admin read it directly; Android mirrors it here.

// base
val Background = Color(0xFF0B0B0D)
val SurfaceDark = Color(0xFF15151A)
val Surface2 = Color(0xFF1B1B21)
val Border = Color(0xFF26262E)
val OnDark = Color(0xFFF3F3F5)
val Muted = Color(0xFFA8A8B3) // reconciled with web/admin (was 0xFF9A9AA6)

// brand
val Accent = Color(0xFF2F6BFF)
val AccentFg = Color(0xFF7DA2FF)

// semantic status palette (mirrors --ok/--warn/--danger/--neutral)
val Ok = Color(0xFF2FAE5A)
val OkBg = Color(0xFF12281C)
val OkFg = Color(0xFF5FD08A)
val Warn = Color(0xFFD9A441)
val WarnBg = Color(0xFF2A2113)
val WarnFg = Color(0xFFE6B860)
val Danger = Color(0xFFE2553A)
val DangerBg = Color(0xFF2E1714)
val DangerFg = Color(0xFFF0816A)
val NeutralBg = Color(0xFF23232B)
val NeutralFg = Color(0xFFB8B8C2)
