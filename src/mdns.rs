//! Bonjour advertisement. Screen Sharing's `SSBonjourBrowser` browses `_rfb._tcp`;
//! `_device-info._tcp` (TXT `model=`) is the standard source of the Finder icon.

use std::collections::HashMap;

use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use tracing::{info, warn};

pub const RFB_SERVICE: &str = "_rfb._tcp.local.";
pub const DEVICE_INFO_SERVICE: &str = "_device-info._tcp.local.";

/// Keeps the advertisement alive; dropping it withdraws the services.
pub struct Advertisement {
    daemon: ServiceDaemon,
    fullnames: Vec<String>,
}

pub fn services(
    instance: &str,
    host: &str,
    port: u16,
    device_model: Option<&str>,
) -> Result<Vec<ServiceInfo>> {
    let host = format!(
        "{}.local.",
        host.trim_end_matches('.').trim_end_matches(".local")
    );
    let mut out = vec![
        ServiceInfo::new(
            RFB_SERVICE,
            instance,
            &host,
            (),
            port,
            HashMap::<String, String>::new(),
        )?
        .enable_addr_auto(),
    ];
    if let Some(model) = device_model {
        let txt = HashMap::from([("model".to_string(), model.to_string())]);
        out.push(
            ServiceInfo::new(DEVICE_INFO_SERVICE, instance, &host, (), 0, txt)?.enable_addr_auto(),
        );
    }
    Ok(out)
}

pub fn advertise(
    instance: &str,
    host: &str,
    port: u16,
    device_model: Option<&str>,
) -> Result<Advertisement> {
    let daemon = ServiceDaemon::new()?;
    let mut fullnames = Vec::new();
    for svc in services(instance, host, port, device_model)? {
        fullnames.push(svc.get_fullname().to_string());
        daemon.register(svc)?;
    }
    info!(?fullnames, "advertising over mDNS");
    Ok(Advertisement { daemon, fullnames })
}

impl Drop for Advertisement {
    fn drop(&mut self) {
        for name in &self.fullnames {
            if let Err(e) = self.daemon.unregister(name) {
                warn!("mDNS unregister {name}: {e}");
            }
        }
        let _ = self.daemon.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_records() {
        let s = services("omarchy (ardd)", "omarchy", 5900, Some("MacPro7,1")).unwrap();
        assert_eq!(s[0].get_fullname(), "omarchy (ardd)._rfb._tcp.local.");
        assert_eq!(s[0].get_port(), 5900);
        assert_eq!(s[0].get_hostname(), "omarchy.local.");
        assert!(s[0].get_properties().is_empty());
        assert_eq!(
            s[1].get_fullname(),
            "omarchy (ardd)._device-info._tcp.local."
        );
        assert_eq!(s[1].get_property_val_str("model"), Some("MacPro7,1"));
        assert_eq!(services("x", "h.local", 1, None).unwrap().len(), 1);
    }
}
