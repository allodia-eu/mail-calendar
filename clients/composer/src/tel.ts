// `mailcal_composer::tel`: which `tel:` URIs this product sends, keeps and opens. RFC 3966's
// grammar, less `*` and `#`, which are service codes in practice and are refused written out and
// percent-encoded, anywhere in the URI.

const NUMBER = /^[0-9().-]*[0-9][0-9().-]*$/;
const LOCAL_NUMBER = /^[0-9a-f().-]*[0-9a-f][0-9a-f().-]*$/i;
const DOMAIN = /^([a-z0-9]([a-z0-9-]*[a-z0-9])?\.)*[a-z]([a-z0-9-]*[a-z0-9])?\.?$/i;
const PNAME = /^[a-z0-9-]+$/i;
const PARAMCHARS = /^(?:[a-z0-9\-_.!~'()[\]/:&+$]|%[0-9a-f]{2})+$/i;
const URICS = /^(?:[a-z0-9\-_.!~'()/?:@&=+$,]|%[0-9a-f]{2})+$/i;
const ENCODED_SERVICE_CHAR = /%(2a|23)/i;

/// `value` less the spaces of a `tel:` number, written as spaces or as `%20`, and unchanged when it
/// is not one. A `tel:` URI holds no spaces (RFC 3966 §5.1.1), so they are dropped rather than
/// refused.
export function withoutNumberSpaces(value: string): string {
  return value.trimStart().slice(0, 4).toLowerCase() === "tel:"
    ? value.replace(/\s/g, "").replace(/%20/g, "")
    : value;
}

/// Whether `subscriber`, what follows `tel:`, is a number RFC 3966 can write with no `*` or `#`:
/// a global number (`+` and digits), or a local one with exactly one `phone-context`.
export function isTelephoneSubscriber(subscriber: string): boolean {
  if (/[*#]/.test(subscriber)) return false;
  const [number = "", ...pars] = subscriber.split(";");
  let contexts = 0;
  for (const par of pars) {
    const at = par.indexOf("=");
    const name = (at < 0 ? par : par.slice(0, at)).toLowerCase();
    const value = at < 0 ? null : par.slice(at + 1);
    let valid: boolean;
    if (name === "phone-context") {
      contexts += 1;
      valid = value !== null && isDescriptor(value);
    } else if (name === "ext") {
      valid = value !== null && NUMBER.test(value);
    } else if (name === "isub") {
      valid = value !== null && isEscaped(value, URICS);
    } else {
      valid = PNAME.test(name) && (value === null || isEscaped(value, PARAMCHARS));
    }
    if (!valid) return false;
  }
  return number.startsWith("+")
    ? contexts === 0 && NUMBER.test(number.slice(1))
    : contexts === 1 && LOCAL_NUMBER.test(number);
}

function isDescriptor(value: string): boolean {
  return value.startsWith("+") ? NUMBER.test(value.slice(1)) : DOMAIN.test(value);
}

function isEscaped(value: string, chars: RegExp): boolean {
  return chars.test(value) && !ENCODED_SERVICE_CHAR.test(value);
}
