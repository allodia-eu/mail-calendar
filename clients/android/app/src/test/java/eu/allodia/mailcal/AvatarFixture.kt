package eu.allodia.mailcal

import uniffi.mailcal_bindings.Avatar
import uniffi.mailcal_bindings.Swatch

/**
 * A fixed avatar for fixtures whose tests do not care which person is drawn.
 *
 * The letters and the colour are the core's to derive, and every suite here is about something
 * else, a swipe, a quote, a merged row. One stub keeps that decision out of them.
 */
fun stubAvatar(initials: String = "SR"): Avatar = Avatar(
    initials = initials,
    light = Swatch(background = "#4C6EF5", text = "#FFFFFF", border = "#3B5BDB"),
    dark = Swatch(background = "#4C6EF5", text = "#FFFFFF", border = "#3B5BDB"),
    imagePath = null,
)
