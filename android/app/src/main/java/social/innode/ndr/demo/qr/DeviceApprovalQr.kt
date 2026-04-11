package social.innode.ndr.demo.qr

import social.innode.ndr.demo.rust.DeviceApprovalQrPayload
import social.innode.ndr.demo.rust.decodeDeviceApprovalQr
import social.innode.ndr.demo.rust.encodeDeviceApprovalQr

object DeviceApprovalQr {
    fun encode(
        ownerInput: String,
        deviceInput: String,
    ): String = encodeDeviceApprovalQr(ownerInput.trim(), deviceInput.trim())

    fun decode(raw: String): DeviceApprovalQrPayload? = decodeDeviceApprovalQr(raw)
}
