//! reqwest owns proxy matching and connections; unsupported settings fail closed.

use reqwest::blocking::Client;
use reqwest::{NoProxy, Proxy, Url};

pub(super) fn client(url: &Url) -> Result<Client, String> {
    let explicit = environment_proxy(url.scheme(), |key| {
        std::env::var(key).map(Some).or_else(|e| match e {
            std::env::VarError::NotPresent => Ok(None),
            std::env::VarError::NotUnicode(_) => Err(format!("{key} is not valid Unicode")),
        })
    })?;
    let builder = if let Some(proxy) = explicit {
        // Explicit environment configuration wins over system preferences,
        // including ALL_PROXY. Never fall back to system/direct on failure.
        super::client_builder()
            .no_proxy()
            .proxy(proxy.no_proxy(NoProxy::from_env()))
    } else {
        check_system_proxy()?;
        super::client_builder()
    };
    builder.build().map_err(super::network_error)
}

fn environment_proxy(
    scheme: &str,
    get: impl Fn(&str) -> Result<Option<String>, String>,
) -> Result<Option<Proxy>, String> {
    let keys = if scheme == "https" {
        ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"]
    } else {
        ["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"]
    };
    for key in keys {
        if let Some(value) = get(key)? {
            // Unlike reqwest's automatic discovery, bad configuration must
            // not silently turn a proxied request into a direct request.
            if value.trim().is_empty() {
                return Err(format!("{key} is empty; unset it or supply a proxy URL"));
            }
            let address = if value.contains("://") {
                value
            } else {
                format!("http://{value}")
            };
            let url =
                Url::parse(&address).map_err(|_| format!("invalid {key}; supply a proxy URL"))?;
            if !matches!(
                url.scheme(),
                "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h"
            ) || url.host_str().is_none()
            {
                return Err(format!("unsupported {key}; use HTTP(S) or SOCKS"));
            }
            return Proxy::all(url)
                .map(Some)
                .map_err(|_| format!("invalid {key}; supply an HTTP(S) or SOCKS proxy URL"));
        }
    }
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
fn check_system_proxy() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn check_system_proxy() -> Result<(), String> {
    use system_configuration::dynamic_store::SCDynamicStoreBuilder;

    let settings = SCDynamicStoreBuilder::new("tm20-images")
        .build()
        .and_then(|store| store.get_proxies())
        .ok_or(
            "cannot read system proxy settings; set HTTPS_PROXY/HTTP_PROXY or ALL_PROXY explicitly",
        )?;
    validate_system_proxy(&settings)?;
    // reqwest ignores all automatic proxies in CGI environments.
    if std::env::var_os("REQUEST_METHOD").is_some() {
        return Err("automatic proxy discovery is disabled in CGI; set an explicit proxy environment variable".into());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
use system_configuration::core_foundation::{
    base::CFType, dictionary::CFDictionary, number::CFNumber, string::CFString,
};

#[cfg(target_os = "macos")]
fn validate_system_proxy(settings: &CFDictionary<CFString, CFType>) -> Result<(), String> {
    for key in [
        "ProxyAutoConfigEnable",
        "ProxyAutoDiscoveryEnable",
        "SOCKSEnable",
    ] {
        let enabled = settings
            .find(CFString::new(key))
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|v| v.to_i32())
            .unwrap_or(0);
        if enabled != 0 {
            return Err(format!(
                "system proxy setting {key} is not supported by automatic discovery; set HTTPS_PROXY/HTTP_PROXY or ALL_PROXY explicitly (socks5h:// for proxy-side DNS)"
            ));
        }
    }
    for protocol in ["HTTP", "HTTPS"] {
        let enabled = settings
            .find(CFString::new(&format!("{protocol}Enable")))
            .and_then(|v| v.downcast::<CFNumber>())
            .and_then(|v| v.to_i32())
            .unwrap_or(0);
        if enabled != 0 {
            let host = settings
                .find(CFString::new(&format!("{protocol}Proxy")))
                .and_then(|v| v.downcast::<CFString>())
                .map(|v| v.to_string());
            let port = settings
                .find(CFString::new(&format!("{protocol}Port")))
                .and_then(|v| v.downcast::<CFNumber>())
                .and_then(|v| v.to_i32());
            if host.as_deref().is_none_or(str::is_empty)
                || port.is_none_or(|p| !(1..=65535).contains(&p))
            {
                return Err(format!(
                    "invalid system {protocol} proxy; fix its host/port or set an explicit proxy environment variable"
                ));
            }
            let host = host.unwrap();
            let address = format!("http://{host}:{}", port.unwrap());
            if !host.is_ascii()
                || Url::parse(&address).ok().is_none_or(|url| {
                    url.host_str()
                        .is_none_or(|parsed| !parsed.eq_ignore_ascii_case(&host))
                        || url.path() != "/"
                        || url.query().is_some()
                        || url.fragment().is_some()
                        || !url.username().is_empty()
                        || url.password().is_some()
                })
            {
                return Err(format!(
                    "invalid system {protocol} proxy host; set an explicit proxy URL"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_or_empty_proxy_is_an_error_not_direct() {
        for value in ["", " ", "http://[", "ftp://proxy.invalid:80"] {
            assert!(
                environment_proxy("https", |key| Ok(
                    (key == "HTTPS_PROXY").then(|| value.into())
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn proxy_environment_is_scheme_specific() {
        assert!(
            environment_proxy("http", |key| Ok(
                (key == "HTTPS_PROXY").then(|| "http://[".into())
            ))
            .unwrap()
            .is_none()
        );
        for key in ["https_proxy", "ALL_PROXY", "all_proxy"] {
            assert!(
                environment_proxy("https", |name| Ok(
                    (name == key).then(|| "socks5h://localhost:1080".into())
                ))
                .unwrap()
                .is_some()
            );
        }
    }

    #[test]
    fn scheme_proxy_precedes_all_proxy() {
        assert!(
            environment_proxy("https", |key| Ok(match key {
                "HTTPS_PROXY" => Some("http://localhost:8080".into()),
                "ALL_PROXY" => Some("http://[".into()),
                _ => None,
            }))
            .unwrap()
            .is_some()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unsupported_system_settings_fail_closed_without_changing_os_preferences() {
        use system_configuration::core_foundation::base::TCFType;
        for key in [
            "ProxyAutoConfigEnable",
            "ProxyAutoDiscoveryEnable",
            "SOCKSEnable",
            "HTTPEnable",
            "HTTPSEnable",
        ] {
            let settings = CFDictionary::from_CFType_pairs(&[(
                CFString::new(key),
                CFNumber::from(1).as_CFType(),
            )]);
            assert!(validate_system_proxy(&settings).is_err(), "{key}");
        }
        assert!(validate_system_proxy(&CFDictionary::from_CFType_pairs(&[])).is_ok());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn manual_system_proxy_host_and_port_are_validated() {
        use system_configuration::core_foundation::base::TCFType;
        for (host, valid) in [
            ("localhost", true),
            ("127.0.0.1", true),
            ("[::1]", true),
            ("::1", false),
            ("bad host", false),
            ("host/path", false),
        ] {
            let settings = CFDictionary::from_CFType_pairs(&[
                (CFString::new("HTTPEnable"), CFNumber::from(1).as_CFType()),
                (CFString::new("HTTPProxy"), CFString::new(host).as_CFType()),
                (CFString::new("HTTPPort"), CFNumber::from(8080).as_CFType()),
            ]);
            assert_eq!(validate_system_proxy(&settings).is_ok(), valid, "{host}");
        }
    }
}
