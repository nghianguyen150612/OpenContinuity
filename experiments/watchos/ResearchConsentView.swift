import CryptoKit
import LocalAuthentication
import OpenContinuityProtocol
import SwiftUI

enum ResearchConsentFailure: Error {
    case cancelled
    case expired
    case unsupported
    case localAuthenticationDenied
    case signingFailed
}

/// Foreground-only view for a request that the parent has already verified
/// with an independently pinned host key. The parent must present this view
/// only after the user explicitly starts the research test.
@MainActor
struct ResearchConsentView: View {
    let verifiedRequest: VerifiedHostRequest
    let watchEnrollment: WatchEnrollment
    /// Passed in memory by a test harness. This view does not persist or export it.
    let researchSigningKey: P256.Signing.PrivateKey
    var diagnostic: (GattDiagnostics.Outcome) -> Void = { _ in }
    let completion: (Result<SignedWatchResponse, ResearchConsentFailure>) -> Void

    @Environment(\.dismiss) private var dismiss
    @State private var authenticationContext: LAContext?
    @State private var isEvaluating = false
    @State private var completed = false
    @State private var status = "Review the request, then choose approve or reject."

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 9) {
                Text("Research confirmation — does not unlock Linux.")
                    .font(.headline)
                    .fixedSize(horizontal: false, vertical: true)

                Text("Host: \(verifiedRequest.request.hostIdentity)")
                    .font(.caption)
                Text("Linux account UID: \(verifiedRequest.request.linuxUID)")
                    .font(.caption)
                Text("Action: \(verifiedRequest.request.action)")
                    .font(.caption)
                Text("Expires: \(verifiedRequest.request.expiresAtMilliseconds) ms since Unix epoch")
                    .font(.caption2)

                Text("A signed app response is research data. It does not prove genuine Watch identity, wearer presence, or proximity.")
                    .font(.caption2)
                    .foregroundStyle(.secondary)

                Button("Approve research confirmation") { approveAfterLocalAuthentication() }
                    .disabled(isEvaluating || completed)
                    .tint(.green)

                Button("Reject research confirmation") { recordManualRejection() }
                    .disabled(isEvaluating || completed)
                    .tint(.red)

                Button("Cancel") { cancel() }
                    .disabled(completed)

                Text(status)
                    .font(.caption2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding()
        }
        .onDisappear {
            if !completed {
                authenticationContext?.invalidate()
                finish(.failure(.cancelled))
            }
        }
    }

    private func approveAfterLocalAuthentication() {
        guard !completed else { return }
        guard currentMilliseconds < verifiedRequest.request.expiresAtMilliseconds else {
            finish(.failure(.expired))
            return
        }

        diagnostic(.notRun) // Starts timing only; never a successful LA result.
        let context = LAContext()
        var policyError: NSError?
        guard context.canEvaluatePolicy(.deviceOwnerAuthentication, error: &policyError) else {
            diagnostic(.blocked)
            status = "watchOS does not support the requested local authentication policy here. No response was signed."
            finish(.failure(.unsupported))
            return
        }

        authenticationContext = context
        isEvaluating = true
        status = "Waiting for local watchOS authentication…"
        context.evaluatePolicy(
            .deviceOwnerAuthentication,
            localizedReason: "Confirm this research request. It will not unlock Linux."
        ) { success, error in
            Task { @MainActor in
                guard !completed else { return }
                isEvaluating = false
                authenticationContext = nil
                diagnostic(success ? .pass : .fail)
                guard success else {
                    if isCancellation(error) {
                        status = "Local authentication was cancelled. No response was signed."
                        finish(.failure(.cancelled))
                    } else {
                        status = "Local authentication was denied or failed. No response was signed."
                        finish(.failure(.localAuthenticationDenied))
                    }
                    return
                }
                signManualDecision(.approved)
            }
        }
    }

    private func recordManualRejection() {
        guard !completed else { return }
        guard currentMilliseconds < verifiedRequest.request.expiresAtMilliseconds else {
            finish(.failure(.expired))
            return
        }
        signManualDecision(.rejected)
    }

    private func signManualDecision(_ decision: WatchAuthDecision) {
        let timestamp = currentMilliseconds
        guard timestamp < verifiedRequest.request.expiresAtMilliseconds else {
            status = "The request expired. No response was signed."
            finish(.failure(.expired))
            return
        }
        do {
            let signed = try WatchAuthProtocol.signWatchResponse(
                verifiedRequest: verifiedRequest,
                enrollment: watchEnrollment,
                signingKey: researchSigningKey,
                decision: decision,
                consentResult: .explicitUserDecision,
                responseAtMilliseconds: timestamp
            )
            status = decision == .approved
                ? "Research approval signed. Linux authorization was never evaluated."
                : "Research rejection signed. Linux authorization was never evaluated."
            finish(.success(signed))
        } catch {
            status = "The response could not be signed. No approval was sent."
            finish(.failure(.signingFailed))
        }
    }

    private func cancel() {
        authenticationContext?.invalidate()
        authenticationContext = nil
        status = "The test was cancelled. No response was signed."
        finish(.failure(.cancelled))
        dismiss()
    }

    private func finish(_ result: Result<SignedWatchResponse, ResearchConsentFailure>) {
        guard !completed else { return }
        completed = true
        completion(result)
    }

    private var currentMilliseconds: UInt64 {
        UInt64(max(0, Date().timeIntervalSince1970 * 1_000))
    }

    private func isCancellation(_ error: Error?) -> Bool {
        guard let error = error as? LAError else { return false }
        return error.code == .userCancel || error.code == .appCancel || error.code == .systemCancel
    }
}
