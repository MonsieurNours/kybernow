#!/bin/bash -e
# Stage KyberNow : part du rootfs du stage précédent (Raspberry Pi OS Lite).
if [ ! -d "${ROOTFS_DIR}" ]; then
	copy_previous
fi
