package social.innode.ndr.demo.account

data class AccountState(
    val publicKeyHex: String,
    val npub: String,
)

data class EncryptedSecret(
    val cipherText: ByteArray,
    val iv: ByteArray,
)

sealed interface AccountBootstrapState {
    data object Loading : AccountBootstrapState
    data object NeedsLogin : AccountBootstrapState
    data class LoggedIn(val account: AccountState) : AccountBootstrapState
}
