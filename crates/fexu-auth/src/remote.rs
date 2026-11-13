//! OIDC, OAuth und Passkeys sind nicht implementiert.
//!
//! Die Grenzen stehen in `docs/auth-threat-model.md`. Es gibt keine Methode,
//! die eine externe Anmeldung als Erfolg meldet.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteProtocol {
    Oidc,
    OAuthAuthorizationCode,
    Passkey,
}

impl RemoteProtocol {
    pub const fn is_implemented(self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::RemoteProtocol;

    #[test]
    fn remote_protocols_are_not_implemented() {
        for protocol in [
            RemoteProtocol::Oidc,
            RemoteProtocol::OAuthAuthorizationCode,
            RemoteProtocol::Passkey,
        ] {
            assert!(!protocol.is_implemented());
        }
    }
}
