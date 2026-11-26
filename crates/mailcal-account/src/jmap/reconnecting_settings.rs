//! The out-of-office reply and the mail rules of [`ReconnectingJmapProvider`]: a read made once
//! more on a provider connected again after a dead connection, a write made once. Offered where
//! the provider connected last offers them, which is where the session lists them.

use async_trait::async_trait;
use engine_core::{
    ids::AccountId,
    rules::{MailRule, RuleSet, RuleVocabulary, RulesReceipt},
    vacation::{VacationReceipt, VacationResponse, VacationVocabulary},
};
use engine_provider::{
    MailRules, Provider, ProviderError, ProviderResult, Vacation, VacationPrecondition, WriteGuard,
};
use provider_jmap::JmapProvider;

use super::reconnecting::ReconnectingJmapProvider;

/// `provider`'s reply, or a failure when the session it was connected with lists none.
fn vacation_of(provider: &JmapProvider) -> ProviderResult<&dyn Vacation> {
    provider.vacation().ok_or_else(|| {
        ProviderError::invalid_state(
            "the account's session no longer offers an out-of-office reply",
        )
    })
}

/// `provider`'s rules, or a failure when the session it was connected with lists none.
fn rules_of(provider: &JmapProvider) -> ProviderResult<&dyn MailRules> {
    provider.mail_rules().ok_or_else(|| {
        ProviderError::invalid_state("the account's session no longer offers mail rules")
    })
}

#[async_trait]
impl Vacation for ReconnectingJmapProvider {
    async fn vocabulary(&self, account: &AccountId) -> ProviderResult<VacationVocabulary> {
        self.provider
            .retrying(|provider| async move { vacation_of(&provider)?.vocabulary(account).await })
            .await
    }

    /// The provider's own guard; a guarded write where it no longer offers a reply, which is the
    /// cautious answer.
    fn vacation_write_guard(&self) -> WriteGuard {
        self.provider
            .last()
            .vacation()
            .map_or(WriteGuard::Enforced, Vacation::vacation_write_guard)
    }

    async fn read_vacation(&self, account: &AccountId) -> ProviderResult<VacationResponse> {
        self.provider
            .retrying(
                |provider| async move { vacation_of(&provider)?.read_vacation(account).await },
            )
            .await
    }

    async fn set_vacation(
        &self,
        account: &AccountId,
        precondition: VacationPrecondition<'_>,
        new: &VacationResponse,
    ) -> ProviderResult<VacationReceipt> {
        self.provider
            .once(|provider| async move {
                vacation_of(&provider)?
                    .set_vacation(account, precondition, new)
                    .await
            })
            .await
    }
}

#[async_trait]
impl MailRules for ReconnectingJmapProvider {
    async fn rule_vocabulary(&self, account: &AccountId) -> ProviderResult<RuleVocabulary> {
        self.provider
            .retrying(|provider| async move { rules_of(&provider)?.rule_vocabulary(account).await })
            .await
    }

    /// The provider's own guard; a guarded write where it no longer offers rules, which is the
    /// cautious answer.
    fn rules_write_guard(&self) -> WriteGuard {
        self.provider
            .last()
            .mail_rules()
            .map_or(WriteGuard::Enforced, MailRules::rules_write_guard)
    }

    async fn read_rules(&self, account: &AccountId) -> ProviderResult<RuleSet> {
        self.provider
            .retrying(|provider| async move { rules_of(&provider)?.read_rules(account).await })
            .await
    }

    async fn replace_rules(
        &self,
        account: &AccountId,
        base: &RuleSet,
        rules: &[MailRule],
    ) -> ProviderResult<RulesReceipt> {
        self.provider
            .once(|provider| async move {
                rules_of(&provider)?
                    .replace_rules(account, base, rules)
                    .await
            })
            .await
    }
}
