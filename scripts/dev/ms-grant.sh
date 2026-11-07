#!/usr/bin/env bash
# Narrow, and put back, the Microsoft Graph permissions one person has granted this app, so the
# re-consent notices can be raised on a real account (docs/provider-oauth.md rule 11): a grant
# without mail writes raises "Sending and managing mail needs your permission" at the first refused
# mark-read, move, delete or send, and one without the calendar raises "Calendar access needs your
# permission" at launch. The notice's Sign in again widens it again.
#
#   az login --allow-no-subscriptions        # once, as a tenant admin; you sign in, not the script
#   scripts/dev/ms-grant.sh show             # the signed-in person's grant to this app
#   scripts/dev/ms-grant.sh narrow           # mail read-only and no calendar; the old scope is saved
#   scripts/dev/ms-grant.sh restore          # put the saved scope back
#
# Revoking the app's consent instead is not the same test: Microsoft then refuses the refresh token
# outright (AADSTS65001), which is a dead sign-in (rule 12), not a narrow one.
#
# It edits the grant the signed-in person gave (`consentType` Principal). A tenant-wide admin grant
# narrows the app for everyone, so it is refused here. The app's registration is
# MAILCAL_MS_CLIENT_ID, from the environment or the repo's `.env` (BUILDING.md).
#
# Microsoft applies a changed grant to newly issued tokens within a minute or two, and the app reads
# what it may open from the grant it stored at the last refresh: restart the app after `narrow`, and
# expect a calendar notice raised from a stale grant to clear only at the next launch.
set -euo pipefail
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib.sh"

# What `narrow` takes away, and what it leaves so mail can still be read: the shape of an account
# connected before the app asked for mail writes and the calendar.
WITHHELD=(Mail.ReadWrite Mail.Send Mail.ReadWrite.Shared Mail.Send.Shared Calendars.ReadWrite)
KEPT_FOR_READING=Mail.Read
GRAPH=https://graph.microsoft.com/v1.0

usage() { die "usage: ms-grant.sh <show|narrow|restore>"; }
[[ $# -eq 1 ]] || usage
command="$1"
case "$command" in show|narrow|restore) ;; *) usage ;; esac

require_cmd az
client_id="$(injected_value MAILCAL_MS_CLIENT_ID)"
[[ -n "$client_id" ]] || die "MAILCAL_MS_CLIENT_ID is not set, in the environment or in .env (BUILDING.md)"

graph() { # <method> <path> [az rest arguments…]
  local method="$1" path="$2"
  shift 2
  az rest --method "$method" --url "$GRAPH/$path" --only-show-errors "$@"
}

az account show --only-show-errors >/dev/null 2>&1 \
  || die "not signed in to the Azure CLI: run 'az login --allow-no-subscriptions' as a tenant admin first"

tenant="$(az account show --only-show-errors --query tenantId -o tsv)"
service_principal="$(graph get "servicePrincipals?\$filter=appId eq '$client_id'&\$select=id" --query 'value[0].id' -o tsv)"
[[ -n "$service_principal" ]] || die "tenant $tenant has no service principal for the app: nobody there has signed in to it yet"
me="$(az ad signed-in-user show --only-show-errors --query id -o tsv)"

# `read` answers non-zero on no input, which is the case the message below is for.
read -r grant scope < <(graph get \
  "oauth2PermissionGrants?\$filter=clientId eq '$service_principal' and principalId eq '$me'" \
  --query "join(' ', [value[0].id, value[0].scope])" -o tsv) || true
[[ -n "${grant:-}" ]] || die "the signed-in person has no grant to the app in tenant $tenant: sign in to the app once, sign the Azure CLI in to the tenant that holds the account ('az login --tenant <tenant> --allow-no-subscriptions'), or the grant is the tenant's own (admin consent), which this does not edit"

state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/mailcal-dev"
saved="$state_dir/ms-grant-$grant.scope"

patch_scope() { # <scope>
  graph patch "oauth2PermissionGrants/$grant"     --headers Content-Type=application/json --body "{\"scope\": \"$1\"}" >/dev/null
  info "the grant now reads: $(graph get "oauth2PermissionGrants/$grant" --query scope -o tsv | xargs)"
}

case "$command" in
  show)
    info "the grant reads: $(xargs <<<"$scope")"
    [[ -f "$saved" ]] && info "a narrowed grant's original is saved at $saved: 'restore' puts it back"
    ;;
  narrow)
    # The original is saved once, so a second narrow cannot overwrite it with the narrowed one.
    mkdir -p "$state_dir"
    [[ -f "$saved" ]] || xargs <<<"$scope" >"$saved"
    narrowed=()
    for permission in $scope; do
      [[ " ${WITHHELD[*]} " == *" $permission "* ]] || narrowed+=("$permission")
    done
    [[ " ${narrowed[*]} " == *" $KEPT_FOR_READING "* ]] || narrowed+=("$KEPT_FOR_READING")
    patch_scope "${narrowed[*]}"
    info "restart the app: the calendar notice is raised at launch, the mail one at the first refused mark-read"
    ;;
  restore)
    [[ -f "$saved" ]] || die "nothing saved to restore: the notice's Sign in again widens the grant as well"
    patch_scope "$(<"$saved")"
    rm -f "$saved"
    ;;
esac
