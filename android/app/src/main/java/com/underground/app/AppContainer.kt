package com.underground.app

import android.content.Context
import com.underground.app.data.AuthApi
import com.underground.app.data.AuthInterceptor
import com.underground.app.data.EventApi
import com.underground.app.data.TokenStore
import com.underground.app.data.UserApi
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import retrofit2.Retrofit
import retrofit2.converter.kotlinx.serialization.asConverterFactory

/** Manual DI: shared singletons (token store + API). Base URL from BuildConfig (§10). */
class AppContainer(context: Context) {
    val tokenStore = TokenStore(context)

    private val client = OkHttpClient.Builder()
        .addInterceptor(AuthInterceptor(tokenStore))
        .build()

    private val json = Json { ignoreUnknownKeys = true }

    private val retrofit = Retrofit.Builder()
        .baseUrl(BuildConfig.API_BASE_URL)
        .client(client)
        .addConverterFactory(json.asConverterFactory("application/json".toMediaType()))
        .build()

    val eventApi: EventApi = retrofit.create(EventApi::class.java)
    val authApi: AuthApi = retrofit.create(AuthApi::class.java)
    val userApi: UserApi = retrofit.create(UserApi::class.java)
}
