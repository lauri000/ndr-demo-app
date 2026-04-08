package social.innode.ndr.demo.qr

import java.net.URI
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.charset.StandardCharsets

data class DeviceApprovalQrPayload(
    val ownerInput: String,
    val deviceInput: String,
)

object DeviceApprovalQr {
    private const val SCHEME = "ndrdemo"
    private const val HOST = "device-link"
    private const val OWNER_PARAM = "owner"
    private const val DEVICE_PARAM = "device"

    fun encode(
        ownerInput: String,
        deviceInput: String,
    ): String {
        val trimmedOwner = ownerInput.trim()
        val trimmedDevice = deviceInput.trim()
        require(trimmedOwner.isNotEmpty()) { "ownerInput must not be blank" }
        require(trimmedDevice.isNotEmpty()) { "deviceInput must not be blank" }
        return buildString {
            append(SCHEME)
            append("://")
            append(HOST)
            append("?")
            append(OWNER_PARAM)
            append("=")
            append(urlEncode(trimmedOwner))
            append("&")
            append(DEVICE_PARAM)
            append("=")
            append(urlEncode(trimmedDevice))
        }
    }

    fun decode(raw: String): DeviceApprovalQrPayload? {
        val trimmed = raw.trim()
        if (trimmed.isEmpty()) {
            return null
        }

        val uri = runCatching { URI(trimmed) }.getOrNull() ?: return null
        if (!uri.scheme.equals(SCHEME, ignoreCase = true)) {
            return null
        }
        if (!uri.host.equals(HOST, ignoreCase = true)) {
            return null
        }

        val query = uri.rawQuery ?: return null
        val params = parseQuery(query)
        val ownerInput = params[OWNER_PARAM]?.trim().takeUnless { it.isNullOrEmpty() } ?: return null
        val deviceInput = params[DEVICE_PARAM]?.trim().takeUnless { it.isNullOrEmpty() } ?: return null

        return DeviceApprovalQrPayload(
            ownerInput = ownerInput,
            deviceInput = deviceInput,
        )
    }

    private fun parseQuery(rawQuery: String): Map<String, String> =
        rawQuery
            .split("&")
            .mapNotNull { pair ->
                val parts = pair.split("=", limit = 2)
                val key = parts.getOrNull(0)?.takeIf { it.isNotBlank() } ?: return@mapNotNull null
                val value = parts.getOrNull(1).orEmpty()
                urlDecode(key) to urlDecode(value)
            }.toMap()

    private fun urlEncode(value: String): String = URLEncoder.encode(value, StandardCharsets.UTF_8.name())

    private fun urlDecode(value: String): String = URLDecoder.decode(value, StandardCharsets.UTF_8.name())
}
