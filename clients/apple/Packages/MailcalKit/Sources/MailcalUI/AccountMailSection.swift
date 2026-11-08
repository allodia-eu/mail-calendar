// An account's mail settings, on its page: a new secret when the stored one stopped working, fetch
// depth, message size, live delivery and watched folders, as the core's page record says them.
// Drawn only while the account is used for mail.

import MailcalBindings
import SwiftUI

struct AccountMailSection: View {
    var model: MailboxModel
    let entry: AccountEntry
    let mail: AccountMailSettings
    @Binding var alert: AccountPageAlert?

    @State private var secret = ""
    @State private var replacing = false
    @State private var replaced = false

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
                ? L10n.signin_expired_prompt(accounts: AccountSettingsLabels.name(entry))
                : L10n.settings_sync_description()
        ) {
            VStack(alignment: .leading, spacing: 14) {
                if replacesSecret || replaced {
                    secretRemedy
                }
                count(
                    L10n.settings_sync_depth_heading(), L10n.settings_sync_depth_description(),
                    mail.depth,
                    label: { $0 == 0 ? L10n.sync_depth_all() : L10n.sync_depth_months(count: Int($0)) }
                ) { .setSyncDepth(account: entry.id, months: $0) }
                count(
                    L10n.settings_message_size_heading(), L10n.settings_message_size_description(),
                    mail.size,
                    label: {
                        $0 == 0
                            ? L10n.message_size_unlimited()
                            : L10n.message_size_megabytes(count: Int($0))
                    }
                ) { .setMessageSizeLimit(account: entry.id, megabytes: $0) }
                Divider()
                arrival
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

    /// A picker over `choice`'s counts, worded by `label`, asking for the count picked.
    private func count(
        _ heading: String, _ note: String?, _ choice: AccountCountChoice,
        label: (UInt16) -> String, intent: @escaping (UInt16) -> AccountPageIntent
    ) -> some View {
        ControlPicker(
            title: heading, note: note, labels: choice.options.map(label),
            selected: Int(choice.selected), control: choice.control
        ) { at in
            model.changeAccount(intent(choice.options[at]))
        }
    }

    /// How mail arrives: as it comes (IMAP IDLE, where the server offers it) or on a timer, and
    /// then the folders watched or the interval.
    @ViewBuilder
    private var arrival: some View {
        if let push = mail.push {
            ControlPicker(
                title: L10n.settings_sync_strategy_label(),
                labels: [L10n.settings_sync_strategy_push(), L10n.settings_sync_strategy_poll()],
                selected: push.on ? 0 : 1, control: push.control
            ) { at in
                model.changeAccount(.setPush(account: entry.id, on: at == 0))
            }
        } else {
            Text(L10n.settings_sync_idle_unsupported())
                .font(.callout)
                .foregroundStyle(.secondary)
        }
        if let interval = mail.interval {
            count(
                L10n.settings_sync_interval_label(), nil, interval,
                label: { L10n.settings_sync_interval_minutes(count: Int($0)) }
            ) { .setPollInterval(account: entry.id, minutes: $0) }
        }
        if mail.push?.on == true {
            VStack(alignment: .leading, spacing: 6) {
                Text(L10n.settings_sync_folders_heading()).font(.subheadline).bold()
                Text(L10n.settings_sync_folders_note(count: Int(mail.watchLimit)))
                    .font(.caption)
                    .foregroundStyle(.secondary)
                ForEach(mail.folders, id: \.key) { folder in
                    // The same word the folder pane uses (docs/folder-pane.md rule 13).
                    ControlToggle(
                        title: folderLabel(role: folder.role, name: folder.name), note: nil,
                        on: folder.on, control: folder.control
                    ) { on in
                        model.changeAccount(
                            .setWatchedFolder(account: entry.id, key: folder.key, on: on))
                    }
                }
            }
        }
    }
}
