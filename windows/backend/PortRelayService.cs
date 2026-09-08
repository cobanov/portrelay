// SPDX-FileCopyrightText: 2026 Muhammed Cobanov
// SPDX-License-Identifier: GPL-3.0-only
using System.Buffers.Binary;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.IO.Pipes;
using System.Net;
using System.Net.Sockets;
using System.Security.Principal;
using System.Text.Json;
using System.Text.RegularExpressions;
using Microsoft.Win32;

namespace Usbipd;

sealed record PortRelayRecovery(string Kind, string Bus, string? Instance, int LocalPort = 0, uint? UsbPort = null);

sealed class PortRelayService : BackgroundService
{
    readonly SemaphoreSlim prepare = new(1);
    readonly SemaphoreSlim connections = new(24);
    readonly ConcurrentDictionary<string, TaskCompletionSource> sessions = new();
    readonly ConcurrentDictionary<string, PortRelayDevice> exports = new();
    readonly ConcurrentQueue<string> errors = new();
    readonly ConcurrentDictionary<long, Task> tasks = new();
    readonly ILogger<PortRelayService> logger;
    readonly string runtime = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData), "PortRelay", "recovery");
    readonly string client = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles), "USBip", "usbip.exe");
    long nextTask;
    public PortRelayService(ILogger<PortRelayService> logger) { this.logger = logger; }

    static async Task<string> Command(string file, IEnumerable<string> arguments, CancellationToken token)
    {
        var start = new ProcessStartInfo(file) { UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true, WorkingDirectory = AppContext.BaseDirectory };
        foreach (var arg in arguments) start.ArgumentList.Add(arg);
        using var process = Process.Start(start) ?? throw new IOException("Could not start the USB driver tool");
        using var cancel = CancellationTokenSource.CreateLinkedTokenSource(token);
        cancel.CancelAfter(TimeSpan.FromSeconds(12));
        using var registration = cancel.Token.Register(() => { try { process.Kill(true); } catch (InvalidOperationException) { } });
        var output = process.StandardOutput.ReadToEndAsync(cancel.Token);
        var error = process.StandardError.ReadToEndAsync(cancel.Token);
        await process.WaitForExitAsync(cancel.Token);
        var text = await output;
        var message = await error;
        if (process.ExitCode != 0) throw new IOException($"USB driver operation failed: {message[..Math.Min(message.Length, 400)]}");
        return text;
    }
    static void Save(string path, PortRelayRecovery recovery)
    {
        var temp = path + ".tmp";
        using (var file = new FileStream(temp, FileMode.Create, FileAccess.Write, FileShare.None, 4096, FileOptions.WriteThrough))
        {
            JsonSerializer.Serialize(file, recovery, PortRelayWire.Json);
            file.Flush(true);
        }
        File.Move(temp, path, true);
    }
    async Task Clean(PortRelayRecovery record, string path)
    {
        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(15));
        if (!PortRelayWire.ValidBus(record.Bus)) throw new InvalidDataException("Invalid recovery record");
        if (record.Kind == "export")
        {
            // Identity is the original Windows instance, never whichever device
            // happens to occupy the old bus/port after a hotplug.
            for (var attempt = 0; attempt < 100; attempt++)
            {
                var current = UsbDevice.GetAll().SingleOrDefault(d => d.InstanceId == record.Instance);
                if (current?.IPAddress is not null || current?.StubInstanceId is not null) { await Task.Delay(100, timeout.Token); continue; }
                if (current?.Guid is Guid guid) UsbipdRegistry.Instance.StopSharingDevice(guid);
                if (current is not null && current.IsForced) throw new IOException("The original USB driver needs recovery");
                File.Delete(path);
                return;
            }
            throw new IOException("Windows has not released the exported USB device");
        }
        if (record.Kind != "import" || record.LocalPort is < 1024 or > 65535)
            throw new InvalidDataException("Invalid import recovery record");
        // A crash can occur before attach prints its assigned port. Match the
        // recorded loopback URL as well as the port, then detach only that lease.
        var url = $"usbip://127.0.0.1:{record.LocalPort}/{record.Bus}";
        for (var attempt = 0; attempt < 30; attempt++)
        {
            var ports = await Command(client, ["port"], timeout.Token);
            var found = Regex.Matches(ports, @"(?ms)^Port\s+(\d+):.*?(?=^Port\s+\d+:|\z)")
                .Cast<Match>().FirstOrDefault(m => m.Value.Split('\n').Any(line => line.Trim().Equals("-> " + url, StringComparison.Ordinal)));
            if (found is null) { File.Delete(path); return; }
            var port = uint.Parse(found.Groups[1].Value);
            if (record.UsbPort.HasValue && record.UsbPort.Value != port) throw new IOException("Virtual port ownership changed; recovery requires attention");
            _ = await Command(client, ["detach", "--port", port.ToString()], timeout.Token);
            await Task.Delay(100, timeout.Token);
        }
        throw new IOException("Windows has not detached the imported USB device");
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        if (!WindowsIdentity.GetCurrent().IsSystem) throw new UnauthorizedAccessException("The USB service requires LocalSystem");
        using var config = Registry.LocalMachine.OpenSubKey(@"SOFTWARE\PortRelay");
        var owner = new SecurityIdentifier(config?.GetValue("OwnerSid") as string ?? throw new IOException("Run PortRelay setup to choose the device owner")).Value;
        if (!Directory.Exists(runtime)) throw new IOException("PortRelay's protected recovery directory is missing");
        foreach (var path in Directory.EnumerateFiles(runtime, "*.json"))
        {
            try { await Clean(JsonSerializer.Deserialize<PortRelayRecovery>(File.ReadAllText(path), PortRelayWire.Json) ?? throw new InvalidDataException(), path); }
            catch (Exception ex) { errors.Enqueue(ex.Message); logger.LogError(ex, "USB recovery requires attention"); }
        }
        var first = true;
        try
        {
            while (!stoppingToken.IsCancellationRequested)
            {
                await connections.WaitAsync(stoppingToken);
                var pipe = PortRelayNative.CreatePipe("PortRelay.Helper.v1", owner, first);
                first = false;
                try { await pipe.WaitForConnectionAsync(stoppingToken); }
                catch { pipe.Dispose(); connections.Release(); throw; }
                if (!PortRelayNative.ClientIs(pipe, owner) && !PortRelayNative.ClientIs(pipe, "S-1-5-18"))
                { pipe.Dispose(); connections.Release(); continue; }
                var id = Interlocked.Increment(ref nextTask);
                tasks[id] = Task.Run(async () =>
                {
                    try { await Handle(pipe, stoppingToken); }
                    catch (Exception ex) when (ex is not OperationCanceledException) { logger.LogDebug(ex, "PortRelay device request ended"); }
                    finally { pipe.Dispose(); connections.Release(); }
                }, CancellationToken.None);
                foreach (var finished in tasks.Where(t => t.Value.IsCompleted).Select(t => t.Key)) tasks.TryRemove(finished, out _);
            }
        }
        catch (OperationCanceledException) when (stoppingToken.IsCancellationRequested) { }
        finally { await Task.WhenAll(tasks.Values); }
    }

    async Task Handle(NamedPipeServerStream pipe, CancellationToken stoppingToken)
    {
        using var preparationTimeout = CancellationTokenSource.CreateLinkedTokenSource(stoppingToken);
        preparationTimeout.CancelAfter(TimeSpan.FromSeconds(18));
        var token = preparationTimeout.Token;
        using var document = await PortRelayWire.Read(pipe, token);
        var request = document.RootElement;
        var op = request.GetProperty("op").GetString();
        var replied = false;
        Stream? data = null;
        Task<string>? attaching = null;
        PortRelayRecovery? recovery = null;
        string? path = null;
        string? key = null;
        var locked = false;
        try
        {
            if (!errors.IsEmpty) throw new IOException("USB recovery required: " + string.Join("; ", errors));
            if (op == "health")
            {
                if (!File.Exists(client)) throw new IOException("Install the Windows USB drivers and restart Windows");
                // Querying the virtual controller proves it is loaded, including
                // driver-signature/required-reboot failures after installation.
                _ = await Command(client, ["port"], token);
                await PortRelayWire.Write(pipe, PortRelayWire.Reply(), token); return;
            }
            if (op == "inventory")
            {
                await prepare.WaitAsync(token); locked = true;
                var devices = (await PortRelayInventory.Read(token)).Select(x => exports.GetValueOrDefault(x.Device.Id, x.Device)).ToList();
                foreach (var active in exports.Values) if (!devices.Any(d => d.Id == active.Id)) devices.Add(active);
                await PortRelayWire.Write(pipe, new { devices, error = (string?)null }, token); return;
            }
            if (op is "wait_export" or "wait_import")
            {
                var waitKey = op == "wait_export" ? "export-" + request.GetProperty("device").GetString() : "import-" + request.GetProperty("port").GetUInt32();
                if (sessions.TryGetValue(waitKey, out var session)) await session.Task.WaitAsync(token);
                if (!errors.IsEmpty) throw new IOException("USB recovery requires a service restart: " + string.Join("; ", errors));
                await PortRelayWire.Write(pipe, PortRelayWire.Reply(), token); return;
            }
            await prepare.WaitAsync(token); locked = true;
            if (op == "export")
            {
                var bus = request.GetProperty("device").GetString() ?? "";
                var generation = request.GetProperty("generation").GetString();
                if (!PortRelayWire.ValidBus(bus)) throw new InvalidDataException("Invalid USB bus ID");
                key = "export-" + bus;
                if (sessions.ContainsKey(key)) throw new IOException("This device is already in use");
                var entry = (await PortRelayInventory.Read(token)).SingleOrDefault(d => d.Device.Id == bus)
                    ?? throw new IOException("Device was unplugged");
                if (entry.Device.Generation != generation) throw new IOException("Device changed; share it again");
                if (entry.Device.Blocked is string reason) throw new IOException(reason);
                PortRelayWire.Validate(entry.Device);
                path = Path.Combine(runtime, key + ".json");
                if (File.Exists(path)) throw new IOException("This device needs recovery");
                recovery = new("export", bus, entry.InstanceId);
                Save(path, recovery);
                sessions[key] = new(TaskCreationOptions.RunContinuationsAsynchronously);
                exports[bus] = entry.Device;
                UsbipdRegistry.Instance.Persist(entry.InstanceId, entry.Device.Name);
                var usb = new NamedPipeClientStream(".", "PortRelay.UsbIp.v1", PipeDirection.InOut, PipeOptions.Asynchronous);
                data = usb;
                await usb.ConnectAsync(token);
                if (!PortRelayNative.ServerIsSystem(usb)) throw new UnauthorizedAccessException("USB pipe owner is not LocalSystem");
                var descriptor = await PortRelayWire.Import(usb, bus, token);
                if (BinaryPrimitives.ReadUInt16BigEndian(descriptor.AsSpan(300)) != Convert.ToUInt16(entry.Device.Vendor, 16)
                    || BinaryPrimitives.ReadUInt16BigEndian(descriptor.AsSpan(302)) != Convert.ToUInt16(entry.Device.Product, 16))
                    throw new IOException("Device identity changed during attachment");
                var device = entry.Device with { Speed = BinaryPrimitives.ReadUInt32BigEndian(descriptor.AsSpan(296)),
                    Devid = (BinaryPrimitives.ReadUInt32BigEndian(descriptor.AsSpan(288)) << 16) | BinaryPrimitives.ReadUInt32BigEndian(descriptor.AsSpan(292)) };
                exports[bus] = device;
                await PortRelayWire.Write(pipe, PortRelayWire.Reply(device: device), token); replied = true;
            }
            else if (op == "import")
            {
                var device = request.GetProperty("device").Deserialize<PortRelayDevice>(PortRelayWire.Json) ?? throw new InvalidDataException("Missing USB metadata");
                PortRelayWire.Validate(device);
                using var listener = new TcpListener(IPAddress.Loopback, 0);
                listener.Start(4);
                var localPort = ((IPEndPoint)listener.LocalEndpoint).Port;
                path = Path.Combine(runtime, $"import-{localPort}.json");
                if (File.Exists(path)) throw new IOException("Virtual connection needs recovery");
                recovery = new("import", device.Id, null, localPort);
                Save(path, recovery);
                attaching = Command(client, ["--tcp-port", localPort.ToString(), "attach", "--remote", "127.0.0.1", "--busid", device.Id, "--terse", "--once"], token);
                TcpClient? kernel = null;
                while (kernel is null)
                {
                    var accepting = listener.AcceptTcpClientAsync(token).AsTask();
                    var finished = await Task.WhenAny(accepting, attaching);
                    if (finished == attaching && !attaching.IsCompletedSuccessfully) await attaching;
                    var candidate = await accepting;
                    var accepted = false;
                    for (var attempt = 0; attempt < 10; attempt++)
                    {
                        if (PortRelayNative.IsKernelConnection((IPEndPoint)candidate.Client.RemoteEndPoint!, (IPEndPoint)candidate.Client.LocalEndPoint!)) { accepted = true; break; }
                        await Task.Delay(10, token);
                    }
                    if (!accepted) { candidate.Dispose(); continue; }
                    kernel = candidate;
                }
                listener.Stop();
                data = kernel.GetStream();
                kernel.NoDelay = true;
                await PortRelayWire.AcceptImport(data, device, token);
                var assigned = (await attaching).Trim();
                if (!uint.TryParse(assigned, out var port) || port is 0 or > 65535) throw new IOException("The driver did not return a virtual port");
                recovery = recovery with { UsbPort = port };
                Save(path, recovery);
                key = "import-" + port;
                if (!sessions.TryAdd(key, new(TaskCreationOptions.RunContinuationsAsynchronously))) throw new IOException("Virtual port is already in use");
                await PortRelayWire.Write(pipe, PortRelayWire.Reply(device: device, port: port), token); replied = true;
            }
            else throw new InvalidDataException("Unknown helper operation");
            prepare.Release(); locked = false;
            preparationTimeout.CancelAfter(Timeout.InfiniteTimeSpan);
            await PortRelayWire.Bridge(pipe, data!, stoppingToken);
        }
        catch (Exception ex)
        {
            if (!replied && pipe.IsConnected)
            {
                using var replyTimeout = new CancellationTokenSource(TimeSpan.FromSeconds(2));
                try { await PortRelayWire.Write(pipe, PortRelayWire.Reply(ex.Message), replyTimeout.Token); } catch (Exception) { }
            }
        }
        finally
        {
            await preparationTimeout.CancelAsync();
            data?.Dispose();
            if (attaching is not null) { try { await attaching; } catch (Exception) { } }
            if (recovery is not null && path is not null)
            {
                if (!locked) { await prepare.WaitAsync(CancellationToken.None); locked = true; }
                try { await Clean(recovery, path); }
                catch (Exception ex) { errors.Enqueue(ex.Message); logger.LogError(ex, "USB cleanup failed"); }
                if (key is not null && sessions.TryRemove(key, out var done)) done.TrySetResult();
                if (recovery.Kind == "export") exports.TryRemove(recovery.Bus, out _);
            }
            if (locked) prepare.Release();
        }
    }
}
