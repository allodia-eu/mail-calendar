// An account's mail settings, on its page: a new secret when the stored one stopped working, the
// name it sends under, fetch depth, message size, live delivery and watched folders. Drawn only
// while the account is used for mail and the core lists its mailbox.

import MailcalBindings
import SwiftUI

struct AccountMailSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let account: AccountSyncRow
    @Binding var alert: AccountPageAlert?

    @State private var secret = ""
    @State private var replacing = false
    @State private var replaced = false

    private var snapshot: SyncSettingsSnapshot? { model.syncSettings }

    /// A pasted password or API token the server stopped accepting, replaced here; an account
    /// whose servers are editable takes its new password in that section instead.
    private var replacesSecret: Bool {
        AccountSettingsRules.replacesSecret(
            entry, expired: model.signInExpired(entry.id),
            provider: model.app?.accountProvider(accountId: entry.id))
    }

    var body: some View {
        SettingsGroup(
            heading: L10n.nav_mail(),
            description: replacesSecret
                ? L10n.signin_expired_prompt(accounts: entry.address)
                : L10n.settings_sync_description()
        ) {
            VStack(alignment: .leading, spacing: 14) {
                if replacesSecret || replaced {
                    secretRemedy
                }
                // The name this account sends under: the one thing here a recipient can see, so
                // it comes before the questions about how much of the account this device keeps
                // (docs/settings.md).
                SenderNameSection(model: model, account: account)
                if let snapshot {
                    picker(
                        L10n.settings_sync_depth_heading(), L10n.settings_sync_depth_description(),
                        L10n.settings_sync_depth_label(), snapshot.syncDepths,
                        Binding(
                            get: { account.syncDepthMonths },
                            set: { model.setAccountSyncDepth(account.accountId, $0) }),
                        label: { $0 == 0 ? L10n.sync_depth_all() : L10n.sync_depth_months(count: Int($0)) }
                    )
                    picker(
                        L10n.settings_message_size_heading(),
                        L10n.settings_message_size_description(),
                        L10n.settings_message_size_label(), snapshot.messageSizeLimitsMb,
                        Binding(
                            get: { account.messageSizeLimitMb },
                            set: { model.setAccountMessageSizeLimit(account.accountId, $0) }),
                        label: {
                            $0 == 0
                                ? L10n.message_size_unlimited()
                                : L10n.message_size_megabytes(count: Int($0))
                        }
                    )
                    Divider()
                    strategy(snapshot)
                }
            }
        }
    }

    /// A new password or API token, tried before it is kept (`replace_account_secret`).
    @ViewBuilder
    private var secretRemedy: some View {
        VStack(alignment: .leading, spacing: 8) {
            SecureField(L10n.setup_jmap_secret_placeholder(), text: $secret)
                .setupField(.password)
                .frame(maxWidth: 320)
            HStack(spacing: 10) {
                Button(L10n.signin_expired_action(), action: replace)
                    .buttonStyle(.borderedProminent)
                    .disabled(secret.isEmpty || replacing)
                if replacing {
                    progress(L10n.status_connecting())
                } else if replaced {
                    Text(L10n.settings_account_signed_in())
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private func replace() {
        let (id, typed) = (entry.id, secret)
        replacing = true
        replaced = false
        Task {
            let failure = await model.replaceAccountSecret(id, typed)
            replacing = false
            if let failure {
                alert = AccountPageAlert(title: L10n.settings_account_signin_failed(), detail: failure)
            } else {
                secret = ""
                replaced = true
            }
        }
    }

    private func picker(
        _ heading: String, _ description: String, _ title: String, _ values: [UInt16],
        _ selection: Binding<UInt16>, label: @escaping (UInt16) -> String
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(heading).font(.subheadline).bold()
            Text(description)
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
            Picker(title, selection: selection) {
                ForEach(values, id: \.self) { value in
                    Text(label(value)).tag(value)
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .frame(maxWidth: 220, alignment: .leading)
        }
    }

    /// Sync behaviour, push (IMAP IDLE, when supported) vs. interval polling.
    @ViewBuilder
    private func strategy(_ snapshot: SyncSettingsSnapshot) -> some View {
        if account.idleSupported {
            Picker(
                L10n.settings_sync_strategy_label(),
                selection: Binding(
                    get: { account.strategy },
                    set: { model.setSyncStrategy(account.accountId, $0) })
            ) {
                Text(L10n.settings_sync_strategy_push()).tag(SyncStrategyKind.push)
                Text(L10n.settings_sync_strategy_poll()).tag(SyncStrategyKind.poll)
            }
            .radioPickerStyle()
        } else {
            Text(L10n.settings_sync_idle_unsupported())
                .font(.callout)
                .foregroundStyle(.secondary)
        }
        switch account.strategy {
        case .push: pushFolders(snapshot)
        case .poll: pollInterval(snapshot)
        }
    }

    private func pollInterval(_ snapshot: SyncSettingsSnapshot) -> some View {
        Picker(
            L10n.settings_sync_interval_label(),
            selection: Binding(
                get: { account.pollIntervalMins },
                set: { model.setPollInterval(account.accountId, $0) })
        ) {
            ForEach(snapshot.pollIntervals, id: \.self) { minutes in
                Text(L10n.settings_sync_interval_minutes(count: Int(minutes))).tag(minutes)
            }
        }
        .frame(maxWidth: 260)
    }

    private func pushFolders(_ snapshot: SyncSettingsSnapshot) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.settings_sync_folders_heading()).font(.subheadline).bold()
            Text(L10n.settings_sync_folders_note(count: Int(snapshot.maxPushFolders)))
                .font(.caption)
                .foregroundStyle(.secondary)
            ForEach(account.folders, id: \.key) { folder in
                Toggle(
                    folderLabel(role: folder.role, name: folder.name),
                    isOn: Binding(
                        get: { folder.subscribed },
                        set: { model.setPushFolder(account.accountId, folder.key, $0) })
                )
                .disabled(!folder.subscribed && account.atPushLimit)
            }
        }
    }
}
