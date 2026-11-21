#!/usr/bin/bash
# Parental controls (the Parental controls app, built with the tour in
# Containerfile.helpers; see build_files/first_steps/src/parental*.rs):
# what its service and its login check need from the system.
set -euox pipefail

# dnsmasq: the family web filter (cosmic-blue-family-dns.service); nftables:
# sends the child accounts' DNS queries to it; acl (setfacl): closes
# systemd-resolved's socket to them; libnotify (notify-send): the warnings
# before the session closes
dnf5 -y install acl dnsmasq libnotify nftables

# The login check: during bedtime, or once the day's screen time is used up,
# the login screen refuses a child account with a message saying when it can
# log in again. Accounts without rules are not affected.
# The login screen logs in through greetd with the PAM service named in
# /etc/greetd/cosmic-greeter.toml ("cosmic-greeter"; "greetd" is greetd's
# default, covered too). Fedora ships these PAM files in /usr/lib/pam.d: a
# copy in /etc/pam.d takes precedence.
# pam_cosmic_blue_parental.so (build_files/first_steps/src/pam_parental.rs)
# runs `cosmic-blue-parental-daemon login-check` and shows its message
line='account    required     pam_cosmic_blue_parental.so'
for service in cosmic-greeter greetd; do
    pam="/etc/pam.d/${service}"
    if [[ ! -f "${pam}" ]]; then
        if [[ ! -f "/usr/lib/pam.d/${service}" ]]; then
            ls /etc/pam.d /usr/lib/pam.d || true
            echo "No PAM file for ${service}" >&2
            exit 1
        fi
        cp "/usr/lib/pam.d/${service}" "${pam}"
    fi
    if ! grep -qF 'pam_cosmic_blue_parental.so' "${pam}"; then
        # Before the first account line, so that it runs whatever follows
        awk -v line="${line}" '!done && /^account/ { print line; done = 1 } { print }' "${pam}" > "${pam}.new"
        grep -qF 'pam_cosmic_blue_parental.so' "${pam}.new"
        cat "${pam}.new" > "${pam}"
        rm -f "${pam}.new"
    fi
    cat "${pam}"
done
grep -qx 'service = "cosmic-greeter"' /etc/greetd/cosmic-greeter.toml \
    || { echo "The login screen's PAM service changed" >&2; exit 1; }
