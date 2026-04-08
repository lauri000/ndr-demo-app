package social.innode.ndr.demo.qr

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DeviceApprovalQrTest {
    @Test
    fun encode_and_decode_round_trip() {
        val owner = "npub1ownerexample"
        val device = "npub1deviceexample"

        val encoded =
            DeviceApprovalQr.encode(
                ownerInput = owner,
                deviceInput = device,
            )

        val decoded = DeviceApprovalQr.decode(encoded)

        requireNotNull(decoded)
        assertEquals(owner, decoded.ownerInput)
        assertEquals(device, decoded.deviceInput)
    }

    @Test
    fun decode_rejects_non_link_input() {
        assertNull(DeviceApprovalQr.decode("npub1plainvalue"))
        assertNull(DeviceApprovalQr.decode("https://example.com"))
    }

    @Test
    fun decode_rejects_missing_fields() {
        assertNull(DeviceApprovalQr.decode("ndrdemo://device-link?owner=npub1owneronly"))
        assertNull(DeviceApprovalQr.decode("ndrdemo://device-link?device=npub1deviceonly"))
    }
}
