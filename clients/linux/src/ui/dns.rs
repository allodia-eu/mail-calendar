//! Native GLib resolver adapter for account autodetection (MX and SRV).

use gtk::gio::{Resolver, ResolverRecordType, prelude::ResolverExt};
use mailcal_bindings::{DnsError, MxRecord, MxResolution, MxResolver, SrvRecord, SrvResolution};

#[derive(Debug, Default)]
pub(super) struct NativeResolver;

impl MxResolver for NativeResolver {
    fn resolve_mx(&self, domain: String) -> Result<MxResolution, DnsError> {
        let records = lookup(&domain, ResolverRecordType::Mx)?
            .into_iter()
            .filter_map(|record| record.get::<(u16, String)>())
            .map(|(preference, exchange)| MxRecord {
                preference,
                exchange: trim_dns_name(&exchange),
            })
            .collect();
        Ok(MxResolution {
            records,
            authentic_data: false,
        })
    }

    fn resolve_srv(&self, name: String) -> Result<SrvResolution, DnsError> {
        let records = lookup(&name, ResolverRecordType::Srv)?
            .into_iter()
            .filter_map(|record| record.get::<(u16, u16, u16, String)>())
            .map(|(priority, weight, port, target)| SrvRecord {
                priority,
                weight,
                port,
                target: trim_dns_name(&target),
            })
            .collect();
        Ok(SrvResolution {
            records,
            authentic_data: false,
        })
    }
}

fn lookup(
    name: &str,
    record_type: ResolverRecordType,
) -> Result<Vec<gtk::glib::Variant>, DnsError> {
    Resolver::default()
        .lookup_records(name, record_type, gtk::gio::Cancellable::NONE)
        .map_err(|error| DnsError::Lookup(error.to_string()))
}

fn trim_dns_name(name: &str) -> String {
    if name == "." {
        name.to_owned()
    } else {
        name.strip_suffix('.').unwrap_or(name).to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::trim_dns_name;

    #[test]
    fn dns_targets_lose_only_the_wire_trailing_dot() {
        assert_eq!(trim_dns_name("mail.example.test."), "mail.example.test");
        assert_eq!(trim_dns_name("."), ".");
        assert_eq!(trim_dns_name("mail.example.test"), "mail.example.test");
    }
}
