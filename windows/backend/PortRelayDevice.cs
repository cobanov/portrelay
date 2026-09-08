// SPDX-FileCopyrightText: 2026 Muhammed Cobanov
// SPDX-License-Identifier: GPL-3.0-only
using System.Security.Cryptography;
using System.Text;
using System.Text.Json.Serialization;
using Windows.Win32;

namespace Usbipd;

sealed partial class WindowsDevice
{
    public string PortRelayParent => TryGetProperty(Node, PInvoke.DEVPKEY_Device_Parent, out string value) ? value : "";
    public string PortRelayClass => TryGetProperty(Node, PInvoke.DEVPKEY_Device_Class, out string value) ? value : "";
    public string PortRelayService => TryGetProperty(Node, PInvoke.DEVPKEY_Device_Service, out string value) ? value : "";
    public string PortRelayArrival => TryGetProperty(Node, PInvoke.DEVPKEY_Device_LastArrivalDate, out var value, out _) ? Convert.ToHexString(value) : "";
}

static class PortRelayInventory
{
    static readonly string Epoch = Guid.NewGuid().ToString();
    public static async Task<List<PortRelayEntry>> Read(CancellationToken token)
    {
        var result = new List<PortRelayEntry>();
        foreach (var usb in UsbDevice.GetAll().Where(d => d.BusId.HasValue && !d.BusId.Value.IsIncompatibleHub).Take(128))
        {
            token.ThrowIfCancellationRequested();
            try
            {
                if (!WindowsDevice.TryCreate(usb.InstanceId, out var windows)) continue;
                var descriptor = await ExportedDevice.GetExportedDevice(usb, token);
                var classes = descriptor.Interfaces.Select(i => i.Item1).Append(descriptor.DeviceClass).ToArray();
                var nodes = windows.Children.Prepend(windows).ToList();
                var imported = false;
                var parent = windows;
                for (var depth = 0; depth < 32; depth++)
                {
                    imported |= parent.InstanceId.Contains("USBIP", StringComparison.OrdinalIgnoreCase)
                        || parent.PortRelayService.Contains("USBIP", StringComparison.OrdinalIgnoreCase);
                    if (!WindowsDevice.TryCreate(parent.PortRelayParent, out parent)) break;
                }
                var network = nodes.Any(n => n.PortRelayClass.Equals("Net", StringComparison.OrdinalIgnoreCase));
                var storage = nodes.Any(n => new[] { "DiskDrive", "Volume", "SCSIAdapter", "WPD" }.Contains(n.PortRelayClass, StringComparer.OrdinalIgnoreCase));
                var input = nodes.Any(n => new[] { "HIDClass", "Keyboard", "Mouse" }.Contains(n.PortRelayClass, StringComparer.OrdinalIgnoreCase));
                var bluetooth = classes.Contains((byte)0xe0) || nodes.Any(n => n.PortRelayClass.Equals("Bluetooth", StringComparison.OrdinalIgnoreCase));
                var blocked = imported ? "Imported devices cannot be shared again"
                    : classes.Contains((byte)9) ? "USB hubs stay on this computer"
                    : classes.Contains((byte)3) || input ? "Input devices stay on this computer in this alpha"
                    : classes.Contains((byte)8) || storage ? "Storage sharing is disabled until recovery is validated"
                    : network ? "Network adapters stay on this computer"
                    : descriptor.Interfaces.Count == 0 && descriptor.DeviceClass == 0 ? "Device interfaces could not be verified"
                    : !classes.Any(c => new byte[] { 2, 7, 10, 0xe0, 0xff }.Contains(c)) ? "This device class is not enabled in this alpha"
                    : usb.Guid.HasValue || usb.IsForced || usb.IPAddress is not null ? "Device is already managed by a USB service"
                    : windows.PortRelayArrival.Length == 0 ? "Device arrival identity could not be verified"
                    : null;
                var generation = Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes($"{Epoch}\0{usb.InstanceId}\0{usb.BusId}\0{windows.PortRelayArrival}")));
                result.Add(new(new(usb.BusId!.Value.ToString(), generation, usb.Description[..Math.Min(usb.Description.Length, 120)],
                    descriptor.VendorId.ToString("x4"), descriptor.ProductId.ToString("x4"), bluetooth ? "bluetooth" : "usb",
                    (uint)descriptor.Speed, ((uint)usb.BusId.Value.Bus << 16) | usb.BusId.Value.Port, blocked), usb.InstanceId));
            }
            catch (Exception ex) when (ex is not OperationCanceledException)
            {
                // Hotplug may invalidate the node while descriptors are queried.
                // An unverified device is never advertised as available.
            }
        }
        return result;
    }
}
