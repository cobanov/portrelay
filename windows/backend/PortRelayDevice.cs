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
    public bool PortRelayUnique => TryGetProperty(Node, PInvoke.DEVPKEY_Device_Capabilities, out uint value) && (value & 0x10) != 0;
}

static class PortRelayInventory
{
    static readonly string Epoch = Guid.NewGuid().ToString();
    static readonly System.Collections.Concurrent.ConcurrentDictionary<string, (string ArrivalGeneration, string PermissionGeneration)> Returned = new();
    public static async Task RememberReturn(PortRelayEntry original)
    {
        if (!original.UniqueIdentity) return;
        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(2));
        try
        {
            for (var attempt = 0; attempt < 20; attempt++)
            {
                var current = (await Read(timeout.Token)).SingleOrDefault(e => e.InstanceId == original.InstanceId);
                if (current is not null)
                {
                    // The Windows exporter itself re-enumerates a device when
                    // returning it. Preserve permission only for a bus-reported
                    // unique identity and unchanged complete USB/IP descriptor.
                    // Any later arrival invalidates this one specific mapping.
                    if (current.UniqueIdentity && current.Fingerprint == original.Fingerprint && current.Device.Blocked is null)
                        Returned[original.InstanceId] = (current.RawGeneration, original.Device.Generation);
                    return;
                }
                await Task.Delay(100, timeout.Token);
            }
        }
        catch (OperationCanceledException) { /* Unverified restoration requires a new grant. */ }
    }
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
                var permissionGeneration = generation;
                if (Returned.TryGetValue(usb.InstanceId, out var returned))
                {
                    if (returned.ArrivalGeneration == generation) permissionGeneration = returned.PermissionGeneration;
                    else Returned.TryRemove(usb.InstanceId, out _);
                }
                using var identity = new MemoryStream();
                descriptor.Serialize(identity, true);
                var fingerprint = Convert.ToHexStringLower(SHA256.HashData(identity.ToArray()));
                result.Add(new(new(usb.BusId!.Value.ToString(), permissionGeneration, usb.Description[..Math.Min(usb.Description.Length, 120)],
                    descriptor.VendorId.ToString("x4"), descriptor.ProductId.ToString("x4"), bluetooth ? "bluetooth" : "usb",
                    (uint)descriptor.Speed, ((uint)usb.BusId.Value.Bus << 16) | usb.BusId.Value.Port, blocked), usb.InstanceId, windows.PortRelayUnique, fingerprint, generation));
            }
            catch (Exception ex) when (ex is not OperationCanceledException)
            {
                // Hotplug may invalidate the node while descriptors are queried.
                // An unverified device is never advertised as available.
            }
        }
        foreach (var instance in Returned.Keys)
            if (!result.Any(e => e.InstanceId == instance)) Returned.TryRemove(instance, out _);
        return result;
    }
}
