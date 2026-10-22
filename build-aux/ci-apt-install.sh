#!/bin/sh
# Install packages on a CI runner, skipping work a throwaway machine never
# needs. Installing libudev-dev upgrades systemd, which then spent half a
# minute rebuilding the initramfs.
set -eu

sudo dpkg-divert --quiet --local --rename --add /usr/sbin/update-initramfs
sudo ln -sf /bin/true /usr/sbin/update-initramfs

if [ "$#" -gt 0 ]; then
    sudo apt-get update -qq
    sudo env DEBIAN_FRONTEND=noninteractive NEEDRESTART_SUSPEND=1 \
        apt-get install -y -qq --no-install-recommends "$@"
fi
