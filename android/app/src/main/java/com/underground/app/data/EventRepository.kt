package com.underground.app.data

import com.underground.app.UndergroundApp

class EventRepository(private val api: EventApi = UndergroundApp.container.eventApi) {
    suspend fun published(): List<EventDto> = api.getEvents()
    suspend fun discover(): List<EventDto> = api.discover()
    suspend fun discoverSections(): List<DiscoverSectionDto> = api.discoverSections()
    suspend fun savedIds(): Set<String> = api.mySaves().map { it.id }.toSet()
    suspend fun save(id: String) = api.saveEvent(id)
    suspend fun unsave(id: String) = api.unsaveEvent(id)
}
