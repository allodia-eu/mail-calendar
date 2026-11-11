// See include/MailcalAgentShim.h.

#if DEBUG
#include "MailcalAgentShim.h"

typedef void (*BoolSetter)(bool, void *__attribute__((swift_context))) __attribute__((swiftcall));

void mailcal_agent_set_bool(const void *setter, void *object, bool value) {
    ((BoolSetter)setter)(value, object);
}
#endif
