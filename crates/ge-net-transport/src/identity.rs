//! Host identity and the pinned-fingerprint verifier.

use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::TransportError;

/// The name in the host certificate. The pin, not this name, is the identity.
pub(crate) const SERVER_NAME: &str = "ge-net-host";

/// SHA-256 of a host certificate in DER form.
///
/// A fingerprint is public: it identifies a host and cannot impersonate one.
/// It should still reach the client over a channel the player trusts.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    fn of(certificate: &[u8]) -> Self {
        Self(Sha256::digest(certificate).into())
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({self})")
    }
}

/// A fingerprint string that is not 64 hexadecimal digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FingerprintParseError;

impl fmt::Display for FingerprintParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a fingerprint is 64 hexadecimal digits")
    }
}

impl std::error::Error for FingerprintParseError {}

impl FromStr for Fingerprint {
    type Err = FingerprintParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let digits = text.as_bytes();
        if digits.len() != 64 {
            return Err(FingerprintParseError);
        }
        let mut bytes = [0; 32];
        for (byte, pair) in bytes.iter_mut().zip(digits.chunks(2)) {
            let pair = std::str::from_utf8(pair).map_err(|_| FingerprintParseError)?;
            *byte = u8::from_str_radix(pair, 16).map_err(|_| FingerprintParseError)?;
        }
        Ok(Self(bytes))
    }
}

/// A host's certificate and private key.
///
/// The private key never leaves this value except into the TLS stack. It is
/// not printed by `Debug` and must not be logged, committed or sent anywhere.
#[derive(Clone)]
pub struct HostIdentity {
    certificate: Vec<u8>,
    private_key: Vec<u8>,
}

impl HostIdentity {
    /// Generate a fresh self-signed identity from the operating system's
    /// secure random source. This never touches a gameplay RNG.
    pub fn generate() -> Result<Self, TransportError> {
        let certified = rcgen::generate_simple_self_signed(vec![SERVER_NAME.to_string()])
            .map_err(|error| TransportError::Tls(error.to_string()))?;
        Ok(Self {
            certificate: certified.cert.der().to_vec(),
            private_key: certified.signing_key.serialize_der(),
        })
    }

    /// Restore an identity from a DER certificate and a PKCS#8 private key
    /// that the application stored itself.
    pub fn from_der(certificate: Vec<u8>, private_key: Vec<u8>) -> Self {
        Self {
            certificate,
            private_key,
        }
    }

    /// The certificate in DER form. Safe to store or share.
    pub fn certificate_der(&self) -> &[u8] {
        &self.certificate
    }

    /// The PKCS#8 private key in DER form, for the application to store
    /// privately. Never log or transmit it.
    pub fn private_key_der(&self) -> &[u8] {
        &self.private_key
    }

    /// What a client pins to reach this host.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of(&self.certificate)
    }

    pub(crate) fn tls_parts(&self) -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
        (
            CertificateDer::from(self.certificate.clone()),
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(self.private_key.clone())),
        )
    }
}

impl fmt::Debug for HostIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostIdentity")
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}

/// Accepts exactly one certificate: the one whose fingerprint was pinned.
///
/// The handshake signature is still verified against that certificate's
/// public key, so a peer must hold the private key, not merely replay the
/// certificate.
#[derive(Debug)]
pub(crate) struct PinnedVerifier {
    pinned: Fingerprint,
    provider: Arc<CryptoProvider>,
}

impl PinnedVerifier {
    pub(crate) fn new(pinned: Fingerprint, provider: Arc<CryptoProvider>) -> Self {
        Self { pinned, provider }
    }
}

impl ServerCertVerifier for PinnedVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if Fingerprint::of(end_entity.as_ref()) == self.pinned {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}
