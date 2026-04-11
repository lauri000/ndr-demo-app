import AVFoundation
import CoreImage.CIFilterBuiltins
import SwiftUI

enum DeviceApprovalQr {
    static func encode(ownerInput: String, deviceInput: String) -> String {
        normalizeDeviceApprovalQr(
            encodeDeviceApprovalQr(
                ownerInput: ownerInput.trimmingCharacters(in: .whitespacesAndNewlines),
                deviceInput: deviceInput.trimmingCharacters(in: .whitespacesAndNewlines)
            )
        )
    }

    static func decode(_ raw: String) -> DeviceApprovalQrPayload? {
        decodeDeviceApprovalQr(raw: raw)
    }
}

private func normalizeDeviceApprovalQr(_ raw: String) -> String {
    raw.trimmingCharacters(in: .whitespacesAndNewlines)
}

struct ResolvedDeviceAuthorizationInput: Equatable {
    let deviceInput: String
    let errorMessage: String?
}

func resolveDeviceAuthorizationInput(
    rawInput: String,
    ownerNpub: String,
    ownerPublicKeyHex: String
) -> ResolvedDeviceAuthorizationInput {
    let trimmed = rawInput.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.isEmpty {
        return ResolvedDeviceAuthorizationInput(deviceInput: "", errorMessage: nil)
    }

    if let payload = DeviceApprovalQr.decode(trimmed) {
        let normalizedOwner = normalizePeerInput(input: payload.ownerInput)
        let acceptedOwnerInputs = Set([
            normalizePeerInput(input: ownerNpub),
            normalizePeerInput(input: ownerPublicKeyHex),
        ])
        if !acceptedOwnerInputs.contains(normalizedOwner) {
            return ResolvedDeviceAuthorizationInput(
                deviceInput: "",
                errorMessage: "This approval QR belongs to a different owner."
            )
        }

        let normalizedDevice = normalizePeerInput(input: payload.deviceInput)
        if !isValidPeerInput(input: normalizedDevice) {
            return ResolvedDeviceAuthorizationInput(
                deviceInput: "",
                errorMessage: "The approval QR did not contain a valid device key."
            )
        }
        return ResolvedDeviceAuthorizationInput(deviceInput: normalizedDevice, errorMessage: nil)
    }

    let normalized = normalizePeerInput(input: trimmed)
    if isValidPeerInput(input: normalized) {
        return ResolvedDeviceAuthorizationInput(deviceInput: normalized, errorMessage: nil)
    }

    return ResolvedDeviceAuthorizationInput(
        deviceInput: "",
        errorMessage: "Not a valid device npub or approval code."
    )
}

struct QrCodeImage: View {
    let text: String

    var body: some View {
        if let image = qrImage(text: text) {
            Image(uiImage: image)
                .interpolation(.none)
                .resizable()
                .scaledToFit()
        } else {
            Color.secondary.opacity(0.1)
                .overlay(Text("QR unavailable").font(.footnote))
        }
    }

    private func qrImage(text: String) -> UIImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.setValue(Data(text.utf8), forKey: "inputMessage")
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else {
            return nil
        }
        let transformed = output.transformed(by: CGAffineTransform(scaleX: 8, y: 8))
        let context = CIContext()
        guard let cgImage = context.createCGImage(transformed, from: transformed.extent) else {
            return nil
        }
        return UIImage(cgImage: cgImage)
    }
}

struct QrScannerSheet: UIViewControllerRepresentable {
    let onCode: (String) -> Void

    func makeUIViewController(context: Context) -> ScannerViewController {
        let controller = ScannerViewController()
        controller.onCode = onCode
        return controller
    }

    func updateUIViewController(_ uiViewController: ScannerViewController, context: Context) {}
}

final class ScannerViewController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var onCode: ((String) -> Void)?

    private let session = AVCaptureSession()
    private var previewLayer: AVCaptureVideoPreviewLayer?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        if let testValue = ProcessInfo.processInfo.environment["NDR_QR_TEST_VALUE"], !testValue.isEmpty {
            DispatchQueue.main.async { [weak self] in
                self?.onCode?(testValue)
            }
            return
        }
        AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
            guard granted else { return }
            DispatchQueue.main.async {
                self?.configureSession()
            }
        }
    }

    private func configureSession() {
        guard previewLayer == nil,
              let device = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: device)
        else {
            return
        }
        if session.canAddInput(input) {
            session.addInput(input)
        }

        let output = AVCaptureMetadataOutput()
        if session.canAddOutput(output) {
            session.addOutput(output)
            output.setMetadataObjectsDelegate(self, queue: .main)
            output.metadataObjectTypes = [.qr]
        }

        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        layer.frame = view.bounds
        view.layer.addSublayer(layer)
        previewLayer = layer
        session.startRunning()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previewLayer?.frame = view.bounds
    }

    func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection
    ) {
        guard let object = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
              let value = object.stringValue
        else {
            return
        }
        session.stopRunning()
        onCode?(value)
    }
}
