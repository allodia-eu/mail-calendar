// What follows a connect on the setup form (docs/onboarding.md, "After the connect"): the link step,
// the accounts an "Add another account" adds on the way, and which of them are asked their name
// once the flow ends. Renderer-free, so the flow is tested without a view; the model holds one and
// the setup screen draws what it says.

import MailcalBindings

/// The last setup step, and a skippable one: an account that lacks a calendar or contacts can use
/// another account's, and a calendar without mail can name the account that sends its invitations.
/// The pickers are Settings' own, from the same snapshot, so the step offers exactly what the
/// account's page would.
struct SetupLinkStep: Equatable {
    /// The account the step links.
    let account: String
    let pickers: [LinkPicker]
    /// Per picker, the index into its options that is picked; `nil` for none. A suggestion starts
    /// picked, and the person confirms it by continuing.
    private(set) var picked: [Int?]
    /// Whether the person has changed a pick, after which a late suggestion leaves the step alone.
    private(set) var touched = false

    private init(account: String, pickers: [LinkPicker]) {
        self.account = account
        self.pickers = pickers
        picked = pickers.map(\.suggested)
    }

    /// The step for `account`, or `nil` when no other account can fill anything it lacks, or its
    /// kind is not offered linking at setup (`offersLinks`, the core's `offers_setup_links`: a
    /// Microsoft or Google account holds everything itself, and linking one is for Settings).
    static func step(
        _ snapshot: AccountsSnapshot, account: String,
        offersLinks: (AccountKind) -> Bool = { offersSetupLinks(kind: $0) }
    ) -> SetupLinkStep? {
        guard let entry = snapshot.accounts.first(where: { $0.id == account }),
              offersLinks(entry.kind)
        else { return nil }
        let pickers = AccountSettingsRules.linkPickers(entry).filter { $0.selected == nil }
        return pickers.isEmpty ? nil : SetupLinkStep(account: account, pickers: pickers)
    }

    /// The same step read again, for a suggestion that arrived after it was drawn. A step the
    /// person has already changed keeps what they picked, carried across by account, slot by
    /// slot; the fresh pickers stand, since they may offer an account added since.
    func refreshed(_ fresh: SetupLinkStep) -> SetupLinkStep {
        guard touched else { return fresh }
        var carried = fresh
        for (index, picker) in fresh.pickers.enumerated() {
            guard let earlier = pickers.firstIndex(where: { $0.slot == picker.slot }) else {
                continue
            }
            let chose = picked[earlier].map { pickers[earlier].options[$0].id }
            carried.picked[index] = chose.flatMap { id in picker.options.firstIndex { $0.id == id } }
        }
        carried.touched = true
        return carried
    }

    /// Records a pick.
    mutating func pick(_ picker: Int, _ option: Int?) {
        guard picked.indices.contains(picker) else { return }
        picked[picker] = option
        touched = true
    }

    /// The links to set: each slot picked, and the account it names.
    var links: [(slot: LinkSlot, target: String)] {
        zip(pickers, picked).compactMap { picker, picked in
            picked.map { (picker.slot, picker.options[$0].id) }
        }
    }
}

struct SetupFlow: Equatable {
    /// The link step on screen; `nil` while the form is on the address or a route.
    private(set) var links: SetupLinkStep?
    /// The account whose link step "Add another account" left, to come back to.
    private(set) var returningTo: String?
    /// The accounts this flow has added so far, in the order added.
    private(set) var added: [String] = []

    /// Whether a flow is under way: an account added, or a link step to come back to.
    var active: Bool { !added.isEmpty || links != nil || returningTo != nil }

    /// An account was added: back to the link step it was added from, on to its own link step when
    /// another account can fill something it lacks, or the end of the flow. Answers whether a link
    /// step is to be shown; otherwise the flow ends.
    mutating func accountAdded(_ account: String, stepFor: (String) -> SetupLinkStep?) -> Bool {
        added.append(account)
        let target = returningTo ?? links?.account ?? account
        returningTo = nil
        links = stepFor(target)
        return links != nil
    }

    /// "Add another account": setup runs again from the address, and comes back here.
    mutating func addLinked() {
        returningTo = links?.account
        links = nil
    }

    /// Cancel, or the form dismissed: back to the link step an "Add another account" left.
    /// Answers whether a link step is to be shown; otherwise the flow ends, and a flow that added
    /// an account ends as a skipped link step does, so each account added is still asked its name.
    mutating func cancelled(stepFor: (String) -> SetupLinkStep?) -> Bool {
        guard let account = returningTo else { return false }
        returningTo = nil
        links = stepFor(account)
        return links != nil
    }

    /// A suggestion the core found after the step was drawn reaches it, unless the person has
    /// changed the step already.
    mutating func refresh(_ fresh: SetupLinkStep?) {
        guard let step = links, let fresh, fresh.account == step.account else { return }
        let refreshed = step.refreshed(fresh)
        if refreshed != step { links = refreshed }
    }

    mutating func pick(_ picker: Int, _ option: Int?) {
        links?.pick(picker, option)
    }

    /// The end of the flow: the accounts to ask a name for, in the order added.
    mutating func finish() -> [String] {
        defer { self = SetupFlow() }
        return added
    }
}
