// The one call the test agent (MailcalUI/TestAgent) cannot make from Swift source: a Swift
// method found by its symbol, called with its object in the register Swift passes `self` in.
// Debug builds only, like the agent.

#if DEBUG
#include <stdbool.h>

/// Calls `setter`, a Swift property setter taking a `Bool`, on `object`.
void mailcal_agent_set_bool(const void *setter, void *object, bool value);
#endif
