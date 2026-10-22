#!/bin/sh
# Install packages on a CI runner, skipping work a throwaway machine never
# needs. Installing libudev-dev upgrades systemd, which then spent half a
# minute rebuilding the initramfs.
set -eu

sudo dpkg-divert --quiet --local --rename --add /usr/sbin/update-initramfs
sudo ln -sf /bin/true /usr/sbin/update-initramfs

# Give up on a stalled mirror sooner and try again. One held a job for six
# minutes. This also covers the apt calls snapcraft makes.
printf '%s\n' \
    'Acquire::Retries "5";' \
    'Acquire::http::Timeout "20";' \
    'Acquire::https::Timeout "20";' |
    sudo tee /etc/apt/apt.conf.d/99ci >/dev/null

if [ "$#" -gt 0 ]; then
    sudo apt-get update -q
    sudo env DEBIAN_FRONTEND=noninteractive NEEDRESTART_SUSPEND=1 \
        apt-get install -y -q --no-install-recommends "$@"
fi
