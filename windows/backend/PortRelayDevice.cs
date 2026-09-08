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
    static IEnumerable<WindowsDevice> Descendants(WindowsDevice root)
    {
        var pending = new Queue<WindowsDevice>(); pending.Enqueue(root);
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        while (pending.Count != 0)
        {
            var node = pending.Dequeue();
            if (!seen.Add(node.InstanceId)) continue;
            if (seen.Count > 256) throw new InvalidDataException("Device tree is too large");
            yield return node;
            foreach (var child in node.Children) pending.Enqueue(child);
        }
    }
    public static async Task<List<PortRelayEntry>> Read(CancellationToken token)
    {
        var result = new List<PortRelayEntry>();
        PortRelayUsage[]? usage = null;
        foreach (var usb in UsbDevice.GetAll().Where(d => d.BusId.HasValue && !d.BusId.Value.IsIncompatibleHub).Take(128))
        {
            token.ThrowIfCancellationRequested();
            try
            {
                if (!WindowsDevice.TryCreate(usb.InstanceId, out var windows)) continue;
                var descriptor = await ExportedDevice.GetExportedDevice(usb, token);
                var classes = descriptor.Interfaces.Select(i => i.Item1).Append(descriptor.DeviceClass).ToArray();
                var nodes = Descendants(windows).ToList();
                var imported = false;
                var parent = windows;
                for (var depth = 0; depth < 32; depth++)
                {
                    imported |= parent.InstanceId.Contains("USBIP", StringComparison.OrdinalIgnoreCase)
                        || parent.PortRelayService.Contains("USBIP", StringComparison.OrdinalIgnoreCase);
                    if (!WindowsDevice.TryCreate(parent.PortRelayParent, out parent)) break;
                }
                var network = nodes.Any(n => n.PortRelayClass.Equals("Net", StringComparison.OrdinalIgnoreCase));
                var storage = classes.Contains((byte)8) || nodes.Any(n => new[] { "DiskDrive", "Volume", "SCSIAdapter" }.Contains(n.PortRelayClass, StringComparer.OrdinalIgnoreCase));
                var input = classes.Contains((byte)3) || nodes.Any(n => new[] { "HIDClass", "Keyboard", "Mouse" }.Contains(n.PortRelayClass, StringComparer.OrdinalIgnoreCase));
                var bluetooth = descriptor.Interfaces.Any(i => i.Item1 == 0xe0 && i.Item2 == 1 && i.Item3 == 1)
                    || nodes.Any(n => n.PortRelayClass.Equals("Bluetooth", StringComparison.OrdinalIgnoreCase));
                var hub = classes.Contains((byte)9);
                if ((storage || network) && usage is null) usage = await PortRelaySafety.Read(token);
                var usageBlock = PortRelaySafety.Blocked(storage, network, nodes.Select(n => n.InstanceId), usage ?? []);
                if (nodes.Any(n => (n.PortRelayClass.Equals("DiskDrive", StringComparison.OrdinalIgnoreCase)
                    || n.PortRelayClass.Equals("Net", StringComparison.OrdinalIgnoreCase))
                    && !(usage ?? []).Any(u => u.Id.Equals(n.InstanceId, StringComparison.OrdinalIgnoreCase))))
                    usageBlock = "Some device functions could not be checked; wait for Windows to finish detecting this device";
                var risks = new[] { (storage, "storage"), (input, "input"), (network, "network"), (bluetooth, "bluetooth") }
                    .Where(r => r.Item1).Select(r => r.Item2).ToArray();
                var blocked = imported ? "Imported devices cannot be shared again"
                    : hub ? "Select the devices connected to this hub"
                    : usageBlock is not null ? usageBlock
                    : descriptor.Interfaces.Count == 0 && descriptor.DeviceClass == 0 ? "Device interfaces could not be verified"
                    : !classes.Any(c => new byte[] { 2, 3, 7, 8, 10, 0xe0, 0xff }.Contains(c)) ? "This device class is not enabled in this alpha"
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
                    descriptor.VendorId.ToString("x4"), descriptor.ProductId.ToString("x4"), hub ? "hub" : bluetooth ? "bluetooth" : storage ? "storage" : input ? "input" : network ? "network" : "usb",
                    (uint)descriptor.Speed, ((uint)usb.BusId.Value.Bus << 16) | usb.BusId.Value.Port, blocked, risks, windows.PortRelayParent.Contains("ROOT_HUB", StringComparison.OrdinalIgnoreCase) ? null : windows.PortRelayParent), usb.InstanceId, windows.PortRelayUnique, fingerprint, generation));
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
