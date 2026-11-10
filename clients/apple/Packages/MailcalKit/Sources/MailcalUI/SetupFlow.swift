// What follows a connect on the setup form (docs/onboarding.md, "After the sign-in" and "After the
// connect"): what a JMAP account is used for, the link step,
// the accounts an "Add another account" adds on the way, and which of them are asked their name
// once the flow ends. Renderer-free, so the flow is tested without a view; the model holds one and
// the setup screen draws what it says.

import MailcalBindings

/// The last setup step, and a skippable one: an account that lacks a calendar or contacts can use
/// another account's, and a calendar without mail can name the account that sends its invitations.
/// The pickers are the core's (`setup_link_pickers`), the ones the account's page offers that
/// nothing fills yet.
struct SetupLinkStep: Equatable {
    /// The account the step links.
    let account: String
    let pickers: [AccountLinkPicker]
    /// The servers found beside the account's JMAP server, offered as an account of their own
    /// for what its session lacks.
    let beside: BesideStep?
    /// Per picker, the index into its options that is picked; `nil` for none. A suggestion starts
    /// picked, and the person confirms it by continuing.
    private(set) var picked: [Int?]
    /// Whether the person has changed a pick, after which a late suggestion leaves the step alone.
    private(set) var touched = false

    private init(account: String, pickers: [AccountLinkPicker], beside: BesideStep?) {
        self.account = account
        self.pickers = pickers
        self.beside = beside
        picked = pickers.map { picker in picker.suggested.map { Int($0) } }
    }

    /// The step for `account`, or `nil` when no other account can fill anything it lacks and its
    /// server offers its calendar and contacts, or its kind is not offered linking at setup
    /// (`offersLinks`, the core's `offers_setup_links`: a Microsoft or Google account holds
    /// everything itself, and linking one is for Settings). A server without one gets the step
    /// anyway, for "Add another account" and for `beside`, which is offered only then.
    static func step(
        _ snapshot: AccountsSnapshot, account: String, beside: BesideOffer? = nil,
        offersLinks: (AccountKind) -> Bool = { offersSetupLinks(kind: $0) },
        pickers: (AccountEntry) -> [AccountLinkPicker] = { setupLinkPickers(entry: $0) }
    ) -> SetupLinkStep? {
        guard let entry = snapshot.accounts.first(where: { $0.id == account }),
              offersLinks(entry.kind)
        else { return nil }
        let pickers = pickers(entry)
        func lacks(_ capability: AccountCapability) -> Bool {
            entry.uses.contains { $0.capability == capability && $0.state == .notOffered }
        }
        let (calendar, contacts) = (lacks(.calendar), lacks(.contacts))
        let offered = beside.flatMap { offer in
            BesideFills(offer.found, lacksCalendar: calendar, lacksContacts: contacts)
                .map { BesideStep(offer: offer, fills: $0) }
        }
        guard !pickers.isEmpty || calendar || contacts else { return nil }
        return SetupLinkStep(account: account, pickers: pickers, beside: offered)
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

/// The step a sign-in ends on when only the server could say what the account offers: a JMAP
/// session offering more than one of mail, calendar and contacts (docs/onboarding.md, "After the
/// sign-in"). The choices are the core's (`signed_in_setup_choices`), drawn with the found card's
/// toggles, so the last use is held on there too.
struct SetupUsesStep: Equatable {
    let account: String
    var choices: UseChoices

    /// The step for `account`, or `nil` when the core offers nothing to choose.
    init?(account: String, choices: [SetupChoice]) {
        guard !choices.isEmpty else { return nil }
        self.account = account
        self.choices = UseChoices(UseOffer(choices: choices))
    }

    /// What the toggles switch, as (use, on): each use chosen that is off now, then each use on
    /// now and not chosen. On first, so the account never passes through being used for nothing.
    var changes: [(capability: AccountCapability, on: Bool)] {
        let offered = choices.offer.choices
        let on = offered.filter { !$0.on && choices.isOn($0.capability) }
            .map { (capability: $0.capability, on: true) }
        let off = offered.filter { $0.on && !choices.isOn($0.capability) }
            .map { (capability: $0.capability, on: false) }
        return on + off
    }
}

struct SetupFlow: Equatable {
    /// The first step after a sign-in, on screen: which of the uses its server offers the account
    /// is used for.
    private(set) var uses: SetupUsesStep?
    /// The account whose link step follows the uses step.
    private var linksAfterUses: String?
    /// The link step on screen; `nil` while the form is on the address or a route.
    private(set) var links: SetupLinkStep?
    /// The step before the link step, on screen: the account's sign-in did not reach the calendar
    /// or contacts chosen for it, so their server is asked for a password of its own
    /// (docs/onboarding.md, "After the connect: a calendar on a server of its own").
    private(set) var separateServer: SeparateServerAsk?
    /// The account whose link step follows the separate-server step.
    private var linksAfterSeparateServer: String?
    /// The account whose link step "Add another account" left, to come back to.
    private(set) var returningTo: String?
    /// The accounts this flow has added so far, in the order added.
    private(set) var added: [String] = []

    /// Whether a flow is under way: an account added, or a link step to come back to.
    var active: Bool {
        !added.isEmpty || links != nil || returningTo != nil || separateServer != nil
            || uses != nil
    }

    /// An account was added: first to the uses step when the core offers `choices`, then to the
    /// separate-server step when `separateServer` names one, then back to the link step it was
    /// added from, on to its own link step when another account can fill something it lacks, or
    /// the end of the flow. Answers whether a step is to be shown; otherwise the flow ends.
    mutating func accountAdded(
        _ account: String, choices: [SetupChoice] = [], separateServer: SeparateServerAsk? = nil,
        stepFor: (String) -> SetupLinkStep?
    ) -> Bool {
        added.append(account)
        let target = returningTo ?? links?.account ?? account
        returningTo = nil
        if let step = SetupUsesStep(account: account, choices: choices) {
            uses = step
            linksAfterUses = target
            links = nil
            return true
        }
        return onward(target, separateServer: separateServer, stepFor: stepFor)
    }

    /// The person changed a toggle on the uses step.
    mutating func choose(_ choices: UseChoices) {
        uses?.choices = choices
    }

    /// The uses step answered and stored: on to the separate-server step when `separateServer`,
    /// read after the uses changed, names one, then to the link step it held back. Answers whether
    /// a step is to be shown; otherwise the flow ends.
    mutating func usesDone(
        separateServer: SeparateServerAsk? = nil, stepFor: (String) -> SetupLinkStep?
    ) -> Bool {
        guard let step = uses else { return links != nil }
        let target = linksAfterUses ?? step.account
        uses = nil
        linksAfterUses = nil
        return onward(target, separateServer: separateServer, stepFor: stepFor)
    }

    private mutating func onward(
        _ target: String, separateServer: SeparateServerAsk?, stepFor: (String) -> SetupLinkStep?
    ) -> Bool {
        if let separateServer {
            self.separateServer = separateServer
            linksAfterSeparateServer = target
            links = nil
            return true
        }
        links = stepFor(target)
        return links != nil
    }

    /// The separate-server step answered, by Connect, Skip or the sign-in: on to the link step
    /// it held back, read afresh so it shows what the answer changed. Answers whether a link step
    /// is to be shown; otherwise the flow ends.
    mutating func separateServerDone(stepFor: (String) -> SetupLinkStep?) -> Bool {
        guard let ask = separateServer else { return links != nil }
        let target = linksAfterSeparateServer ?? ask.account
        separateServer = nil
        linksAfterSeparateServer = nil
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
