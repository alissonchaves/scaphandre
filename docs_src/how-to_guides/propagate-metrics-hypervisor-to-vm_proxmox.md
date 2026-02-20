# Propagate power consumption metrics from Proxmox host to virtual machines

## Introduction

This guide explains how to expose VM-specific power metrics from a Proxmox host running Scaphandre to guest VMs.

The expected flow is:

1. Run Scaphandre on the Proxmox host with the Proxmox exporter.
2. Expose the VM-specific metrics directory to each VM.
3. Mount that directory inside the guest.
4. Run Scaphandre in VM mode in the guest.

## How to

### 1. Start Scaphandre on the Proxmox host

Run Scaphandre with the Proxmox exporter:

    scaphandre proxmox

This exporter writes VM folders under:

    /tmp/scaphandre

Each VM directory name follows:

    <VMID>-<VM_NAME>

Create the VM directory manually before configuring passthrough:

    mkdir -p /tmp/scaphandre/<VMID>-<VM_NAME>

### 2. Map the host folder into the VM (Proxmox config)

On the Proxmox host, edit the VM config:

    /etc/pve/qemu-server/<VMID>.conf

Add a passthrough entry using a fixed mount tag (`scaphandre`) and the VM folder created by the exporter:

    args: -virtfs local,path=/tmp/scaphandre/<VMID>-<VM_NAME>,mount_tag=scaphandre,security_model=passthrough,readonly=on

Then restart the VM.

### 3. Mount inside the guest VM

Inside the VM:

    mkdir -p /var/scaphandre
    mount -t 9p -o trans=virtio,ro scaphandre /var/scaphandre

To persist the mount across reboots, add this line to `/etc/fstab`:

    scaphandre  /var/scaphandre  9p  trans=virtio,ro,_netdev,nofail,x-systemd.automount  0  0

Then apply:

    mkdir -p /var/scaphandre
    mount -a

### 4. Run Scaphandre inside the guest

Run Scaphandre in VM mode with the exporter you want:

    scaphandre --vm prometheus

If you mount on a different path, set:

    SCAPHANDRE_POWERCAP_PATH=/your/path scaphandre --vm prometheus

## Notes

- The VM folder name must match the directory produced by the host exporter.
- This setup is read-only on the guest side.
