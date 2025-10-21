// End-To-End Encryption
use mls_rs::{
	time::MlsTime,
	MlsMessage,
    Client,
	Group,
	ReceivedMessage,
	client_builder::MlsConfig,
    identity::{SigningIdentity, basic::{BasicIdentityProvider, BasicCredential}},
    CipherSuite,
	ExtensionList
};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
use mls_rs_crypto_openssl::OpensslCryptoProvider;

pub fn build_client(name: String) -> Client<impl MlsConfig> {
	use rand_core::OsRng;

	let mut crng = OsRng;
	let signer = SigningKey::generate(&mut crng);
	let verifier = signer.verifying_key();
	let secret = signer.to_bytes().to_vec().into();
	let public = verifier.to_bytes().to_vec().into();

	let identity = BasicCredential::new(name.as_bytes().to_vec());
	let s_identity = SigningIdentity::new(identity.into_credential(), public);

	mls_rs::Client::builder()
		.crypto_provider(OpensslCryptoProvider::default())
		.identity_provider(BasicIdentityProvider::new())
		.signing_identity(s_identity, secret, CipherSuite::CURVE25519_AES128)
		.build()
}

