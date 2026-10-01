//! Forgetting one domain of an account (its mail, its calendars or its contacts) once it is no
//! longer used for it.

use engine_api::{AccountId, Provider, SearchDomain};

use crate::App;

impl<P: Provider> App<P> {
    /// Forgets what the store holds of `account`'s `domain`, and rebuilds the view that showed
    /// it. The account's other domains are left as they are, so their next sync is a delta.
    ///
    /// Call it once the account has been re-installed without that domain's providers, so
    /// nothing syncs into what is being forgotten.
    pub async fn forget_account_domain(&self, account: &AccountId, domain: SearchDomain) {
        if let Err(err) = self.engine.forget_account_domain(account, domain).await {
            log::warn!("forget-domain: the store could not forget a {domain:?} domain: {err}");
        }
        match domain {
            SearchDomain::Mail => {
                self.clear_mail_reauth_required(account);
                self.invalidate_list_cache();
                self.rebuild_snapshot().await;
            }
            SearchDomain::Calendar => {
                self.clear_calendar_reauth_required(account);
                self.rebuild_calendar_view().await;
            }
            SearchDomain::Contacts => self.rebuild_contacts().await,
        }
        self.reclaim_freed_space("forget-domain").await;
    }
}
