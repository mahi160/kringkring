use std::{error::Error, net::Ipv4Addr};

use axum_server::tls_rustls::RustlsConfig;
use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use time::{Duration, OffsetDateTime};

pub async fn generate(ip: Ipv4Addr) -> Result<(Vec<u8>, RustlsConfig), Box<dyn Error>> {
    let now = OffsetDateTime::now_utc();
    let mut root = CertificateParams::new(vec![])?;
    root.distinguished_name
        .push(DnType::CommonName, "Kringkring Spike CA");
    root.is_ca = IsCa::Ca(BasicConstraints::Constrained(0));
    root.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    root.not_before = now - Duration::days(1);
    root.not_after = now + Duration::days(3650);
    let root_key = KeyPair::generate()?;
    let ca = root.self_signed(&root_key)?;
    let issuer = Issuer::new(root, root_key);

    let mut leaf = CertificateParams::new(vec![ip.to_string()])?;
    leaf.distinguished_name
        .push(DnType::CommonName, ip.to_string());
    leaf.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    leaf.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    leaf.use_authority_key_identifier_extension = true;
    leaf.not_before = now - Duration::days(1);
    leaf.not_after = now + Duration::days(30);
    let leaf_key = KeyPair::generate()?;
    let cert = leaf.signed_by(&leaf_key, &issuer)?;
    let tls = RustlsConfig::from_der(vec![cert.der().to_vec()], leaf_key.serialize_der()).await?;
    Ok((ca.der().to_vec(), tls))
}
