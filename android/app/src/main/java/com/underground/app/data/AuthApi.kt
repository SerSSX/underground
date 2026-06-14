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

@Serializable
data class KycViewDto(
    @SerialName("kyc_status") val kycStatus: String,
    @SerialName("has_bank_details") val hasBankDetails: Boolean,
)

@Serializable
data class KycReq(val name: String, val iban: String, val swift: String)

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

interface AuthApi {
    @POST("api/auth/register")
    suspend fun register(@Body req: RegisterReq): AuthResp

    @POST("api/auth/login")
    suspend fun login(@Body req: LoginReq): AuthResp

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
    @GET("api/me/kyc")
    suspend fun getKyc(): KycViewDto

    @POST("api/me/kyc")
    suspend fun submitKyc(@Body req: KycReq)

    // Abuse reporting (P1 #76)
    @POST("api/reports")
    suspend fun createReport(@Body req: CreateReportReq)
}
