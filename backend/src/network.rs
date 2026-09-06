use crate::{config::Config, error::{AppError, Result}};
use std::{net::{IpAddr, SocketAddr}, time::Duration};
use url::Url;

pub fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let n = u32::from(v);
            let blocked = ["0.0.0.0/8", "10.0.0.0/8", "100.64.0.0/10", "127.0.0.0/8", "169.254.0.0/16", "172.16.0.0/12", "192.0.0.0/24", "192.0.2.0/24", "192.168.0.0/16", "198.18.0.0/15", "198.51.100.0/24", "203.0.113.0/24", "224.0.0.0/3"];
            n != u32::from(std::net::Ipv4Addr::new(168,63,129,16)) && !blocked.iter().any(|b| b.parse::<ipnet::Ipv4Net>().expect("constant network").contains(&v))
        }
        IpAddr::V6(v) => {
            "2000::/3".parse::<ipnet::Ipv6Net>().expect("constant network").contains(&v)
                && !["2001::/32", "2001:db8::/32", "2002::/16"].iter().any(|b| b.parse::<ipnet::Ipv6Net>().expect("constant network").contains(&v))
        }
    }
}
pub fn parse_url(raw: &str, cfg: &Config) -> Result<Url> {
    if raw.len() > 4096 { return Err(AppError::bad("Destination URL too long")); }
    let u = Url::parse(raw).map_err(|_| AppError::bad("Invalid destination URL"))?;
    if !u.username().is_empty() || u.password().is_some() || u.fragment().is_some() || u.host_str().is_none() {
        return Err(AppError::bad("Destination URL cannot contain credentials or fragment"));
    }
    if u.scheme() != "https" && !(u.scheme() == "http" && cfg.allow_insecure_notifications && cfg.private_hosts.contains(u.host_str().unwrap_or_default())) {
        return Err(AppError::bad("HTTPS is required for notification destinations"));
    }
    Ok(u)
}
pub async fn resolve(host: &str, port: u16, cfg: &Config) -> Result<Vec<SocketAddr>> {
    let host = host.trim_matches(['[', ']']).to_ascii_lowercase();
    let addresses: Vec<_> = tokio::time::timeout(Duration::from_secs(5), tokio::net::lookup_host((host.as_str(),port))).await
        .map_err(|_| AppError::bad("Destination DNS timeout"))?.map_err(|_| AppError::bad("Destination DNS resolution failed"))?.collect();
    if addresses.is_empty() || addresses.len() > 32 { return Err(AppError::bad("Invalid destination DNS response")); }
    let trusted = cfg.private_hosts.contains(&host);
    for a in &addresses {
        if !trusted && !public_ip(a.ip()) { return Err(AppError::bad("Non-public destination blocked; an administrator must explicitly allow this host")); }
        // Cloud metadata is never an allowed destination, even in private-network mode.
        if a.ip().to_string() == "169.254.169.254" || a.ip().to_string() == "168.63.129.16" { return Err(AppError::bad("Metadata endpoint blocked")); }
    }
    Ok(addresses)
}
pub async fn client(u: &Url, cfg: &Config) -> Result<reqwest::Client> {
    let host = u.host_str().ok_or_else(|| AppError::bad("Missing destination host"))?;
    let addresses = resolve(host, u.port_or_known_default().ok_or_else(|| AppError::bad("Missing port"))?, cfg).await?;
    reqwest::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(18)).connect_timeout(Duration::from_secs(5))
        .resolve_to_addrs(host, &addresses).user_agent("CardDue/0.1").build().map_err(|_| AppError::internal())
}
pub async fn small_body(mut response: reqwest::Response) -> Result<Vec<u8>> {
    if response.content_length().unwrap_or(0) > 65536 { return Err(AppError::bad("Provider response exceeds limit")); }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| AppError::bad("Provider response could not be read"))? {
        if body.len() + chunk.len() > 65536 { return Err(AppError::bad("Provider response exceeds limit")); }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn block_reserved_and_metadata() {
        for s in ["127.0.0.1", "10.0.0.1", "169.254.169.254", "168.63.129.16", "192.168.1.1", "100.64.1.1", "::1", "::ffff:127.0.0.1", "fd00::1", "2001:db8::1"] { assert!(!public_ip(s.parse().unwrap()), "{s}"); }
        assert!(public_ip("8.8.8.8".parse().unwrap()));
        assert!(public_ip("2606:4700:4700::1111".parse().unwrap()));
    }
}
