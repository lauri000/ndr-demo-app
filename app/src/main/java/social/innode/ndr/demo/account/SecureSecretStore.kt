package social.innode.ndr.demo.account

interface SecureSecretStore {
    fun encrypt(secret: ByteArray): EncryptedSecret

    fun decrypt(encryptedSecret: EncryptedSecret): ByteArray
}
