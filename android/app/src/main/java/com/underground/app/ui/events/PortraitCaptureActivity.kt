package com.underground.app.ui.events

import com.journeyapps.barcodescanner.CaptureActivity

/**
 * Portrait-locked QR scanner host (the library's default rotates to landscape).
 * Orientation is pinned in the manifest; the system back button cancels and returns.
 */
class PortraitCaptureActivity : CaptureActivity()
