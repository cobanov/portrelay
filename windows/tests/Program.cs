// SPDX-FileCopyrightText: 2026 Muhammed Cobanov
// SPDX-License-Identifier: GPL-3.0-only
using System.Buffers.Binary;
using System.Text;
using Usbipd;

static void Check(bool condition, string message) { if (!condition) throw new Exception(message); }
static async Task Reject(Func<Task> action) { try { await action(); } catch (InvalidDataException) { return; } catch (EndOfStreamException) { return; } throw new Exception("Invalid input was accepted"); }
var token = CancellationToken.None;
var device = new PortRelayDevice("2-7", "generation", "Serial fixture", "1d6b", "0104", "usb", 3, 0x00020007, null);
var request = new byte[40];
new byte[] {1,0x11,0x80,3,0,0,0,0}.CopyTo(request, 0);
Encoding.ASCII.GetBytes("2-7").CopyTo(request,8);
using (var wire = new MemoryStream())
{
    wire.Write(request); wire.Position = 0;
    await PortRelayWire.AcceptImport(wire, device, token);
    var response = wire.ToArray().AsSpan(40).ToArray();
    Check(response.Length == 320, "USB/IP import reply must contain exactly 8+312 bytes");
    Check(response.AsSpan(0,8).SequenceEqual(new byte[] {1,0x11,0,3,0,0,0,0}), "USB/IP header mismatch");
    Check(Encoding.ASCII.GetString(response,264,32).TrimEnd('\0') == "2-7", "Bus ID offset mismatch");
    Check(BinaryPrimitives.ReadUInt32BigEndian(response.AsSpan(296)) == 2, "Bus number must be network order");
    Check(BinaryPrimitives.ReadUInt32BigEndian(response.AsSpan(300)) == 7, "Device number must be network order");
    Check(BinaryPrimitives.ReadUInt32BigEndian(response.AsSpan(304)) == 3, "Speed offset mismatch");
    Check(BinaryPrimitives.ReadUInt16BigEndian(response.AsSpan(308)) == 0x1d6b, "Vendor offset mismatch");
    Check(BinaryPrimitives.ReadUInt16BigEndian(response.AsSpan(310)) == 0x0104, "Product offset mismatch");
}
foreach (var invalid in new[] { device with { Id = "2-7\0other" }, device with { Id = "../7" }, device with { Speed = 4 }, device with { Devid = 2 }, device with { Vendor = "gggg" } })
    await Reject(() => { PortRelayWire.Validate(invalid); return Task.CompletedTask; });
var wrongBus = (byte[])request.Clone(); wrongBus[8] = (byte)'3';
await Reject(() => PortRelayWire.AcceptImport(new MemoryStream(wrongBus), device, token));
var wrongCommand = (byte[])request.Clone(); wrongCommand[3] = 5;
await Reject(() => PortRelayWire.AcceptImport(new MemoryStream(wrongCommand), device, token));
await Reject(async () => { using var _ = await PortRelayWire.Read(new MemoryStream(new byte[] {0,1,0,1}), token); });
await Reject(async () => { using var _ = await PortRelayWire.Read(new MemoryStream(new byte[] {0,0,0,8,(byte)'{'}), token); });
using (var frames = new MemoryStream())
{
    await PortRelayWire.Write(frames, PortRelayWire.Reply(device:device,port:5), token);
    frames.Position = 0;
    using var json = await PortRelayWire.Read(frames, token);
    Check(json.RootElement.GetProperty("device").GetProperty("devid").GetUInt32() == 0x20007, "Agent JSON compatibility");
    Check(json.RootElement.GetProperty("port").GetUInt32() == 5, "Port JSON compatibility");
}
Console.WriteLine("Windows USB/IP wire conformance, invalid metadata, truncated frames, and agent JSON: passed");

Check(PortRelaySafety.Blocked(true, false, ["USBSTOR\\DISK"], []) is not null, "Unknown disk usage must fail closed");
Check(PortRelaySafety.Blocked(true, false, ["USBSTOR\\DISK"], [new("usbstor\\disk", "storage", true)]) is null, "Offline disk should be eligible");
Check(PortRelaySafety.Blocked(true, false, ["a", "b"], [new("a", "storage", true), new("b", "storage", false)]) is not null, "Every LUN must be offline");
Check(PortRelaySafety.Blocked(false, true, ["adapter"], [new("adapter", "network", false)]) is not null, "Active network adapter must stay local");
Check(PortRelaySafety.Blocked(true, true, ["disk", "net"], [new("disk", "storage", true), new("net", "network", true)]) is null, "Composite idle interfaces should be eligible");
Console.WriteLine("Disk and network usage guards: passed");

if (OperatingSystem.IsWindows())
{
    var usage = await PortRelaySafety.Read(token);
    Check(usage.Any(u => u.Kind == "storage" && !u.Safe), "Live Windows provider must detect an online disk");
    Check(usage.Any(u => u.Kind == "network" && !u.Safe), "Live Windows provider must detect an active network adapter");
    Console.WriteLine("Read-only Windows disk/network provider query: passed");
}
