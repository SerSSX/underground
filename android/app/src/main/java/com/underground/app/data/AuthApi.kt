package com.underground.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonElement
import retrofit2.http.Body
import retrofit2.http.GET
import retrofit2.http.HTTP
import retrofit2.http.PATCH
import retrofit2.http.POST

@Serializable
data class UserDto(
    val id: String,
    val email: String,
    val name: String,
    val role: String,
    @SerialName("avatar_url") val avatarUrl: String? = null,
    val bio: String? = null,
    @SerialName("is_organizer") val isOrganizer: Boolean = false,
    /** Date of birth YYYY-MM-DD (self-view; drives age-gating, P1 #77). */
    val dob: String? = null,
)

@Serializable
data class PatchMeReq(
    val name: String? = null,
    val bio: String? = null,
    @SerialName("avatar_url") val avatarUrl: String? = null,
    @SerialName("is_organizer") val isOrganizer: Boolean? = null,
    /** Date of birth YYYY-MM-DD (age-gating, P1 #77). */
    val dob: String? = null,
)

@Serializable
data class DeleteMeReq(val password: String)

/** Whether the organiser's account at the payment provider can take money yet (PAY-5). */
@Serializable
data class PayoutStatusDto(
    @SerialName("charges_enabled") val chargesEnabled: Boolean,
    @SerialName("payouts_enabled") val payoutsEnabled: Boolean,
    @SerialName("requirements_due") val requirementsDue: List<String> = emptyList(),
    val connected: Boolean,
    val country: String? = null,
    @SerialName("kyc_status") val kycStatus: String,
)

/** The organiser declares only their country; identity and bank details are entered on the
 *  provider's own pages and never pass through the app. */
@Serializable
data class ConnectPayoutsReq(val country: String)

@Serializable
data class ConnectPayoutsResp(@SerialName("onboarding_url") val onboardingUrl: String)

@Serializable
data class CreateReportReq(
    @SerialName("target_type") val targetType: String,
    @SerialName("target_id") val targetId: String,
    val reason: String,
    val details: String? = null,
)

@Serializable
data class RegisterReq(val email: String, val password: String, val name: String)

@Serializable
data class LoginReq(val email: String, val password: String)

@Serializable
data class ForgotPasswordReq(val email: String)

@Serializable
data class AuthResp(val token: String, val user: UserDto)

// Passwordless email one-time code (TASK-115, WF-Auth).
@Serializable
data class RequestCodeReq(val email: String)

@Serializable
data class VerifyCodeReq(val email: String, val code: String)

interface AuthApi {
    @POST("api/auth/register")
    suspend fun register(@Body req: RegisterReq): AuthResp

    @POST("api/auth/login")
    suspend fun login(@Body req: LoginReq): AuthResp

    // Passwordless: request a one-time code, then verify it to sign in (account created on first use).
    @POST("api/auth/email/request-code")
    suspend fun requestEmailCode(@Body req: RequestCodeReq)

    @POST("api/auth/email/verify-code")
    suspend fun verifyEmailCode(@Body req: VerifyCodeReq): AuthResp

    // Password reset (structural bet): request a link; the reset itself completes on the web app.
    @POST("api/auth/forgot-password")
    suspend fun forgotPassword(@Body req: ForgotPasswordReq)

    @GET("api/me")
    suspend fun me(): UserDto

    @PATCH("api/me")
    suspend fun patchMe(@Body req: PatchMeReq): UserDto

    // GDPR (P1 #73)
    @GET("api/me/export")
    suspend fun exportMe(): JsonElement

    @HTTP(method = "DELETE", path = "api/me", hasBody = true)
    suspend fun deleteMe(@Body req: DeleteMeReq)

    // Organizer KYC (P1 #78)
    @GET("api/me/payouts/status")
    suspend fun getPayoutStatus(): PayoutStatusDto

    @POST("api/me/payouts/connect")
    suspend fun connectPayouts(@Body req: ConnectPayoutsReq): ConnectPayoutsResp

    // Abuse reporting (P1 #76)
    @POST("api/reports")
    suspend fun createReport(@Body req: CreateReportReq)
}
