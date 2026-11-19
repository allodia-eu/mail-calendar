// The stored credential entries that are the core's own rather than a mail account: the Allodia
// grant and the own AI endpoint's key share the Keychain with the accounts under reserved ids.
// Which entry is which is the core's question (`is_reserved_config`), never a shape matched here.

import MailcalBindings

/// Whether `configs` hold no mail account, which is what a first run is: an Allodia sign-in or a
/// saved endpoint key made before any mailbox was added is not one.
func holdsNoMailAccount(_ configs: [String]) -> Bool {
    configs.allSatisfy { isReservedConfig(config: $0) }
}

/// The entries in `configs` that are not mail accounts, which a debug launch connecting a canned
/// account carries over so a sign-in or a key saved in that mode sticks.
func reservedConfigs(_ configs: [String]) -> [String] {
    configs.filter { isReservedConfig(config: $0) }
}
