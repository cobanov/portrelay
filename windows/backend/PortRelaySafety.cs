// SPDX-FileCopyrightText: 2026 Muhammed Cobanov
// SPDX-License-Identifier: GPL-3.0-only
using System.Diagnostics;
using System.Text;
using System.Text.Json;

namespace Usbipd;

sealed record PortRelayUsage(string Id, string Kind, bool Safe);
static class PortRelaySafety
{
    // Fixed, read-only query. No device name, peer input or user command is
    // interpolated. An unavailable provider is a closed gate, never permission.
    const string Query = """
        $ErrorActionPreference = 'Stop'
        $result = @()
        foreach ($drive in @(Get-CimInstance Win32_DiskDrive)) {
            $disk = Get-Disk -Number $drive.Index -ErrorAction SilentlyContinue
            $result += @{ Id = $drive.PNPDeviceID; Kind = 'storage'; Safe = [bool]($disk -and $disk.IsOffline -and !$disk.IsBoot -and !$disk.IsSystem) }
        }
        foreach ($adapter in @(Get-NetAdapter -IncludeHidden)) {
            $result += @{ Id = $adapter.PnPDeviceID; Kind = 'network'; Safe = [bool]($adapter.InterfaceAdminStatus -eq 2) }
        }
        ConvertTo-Json -InputObject @($result) -Compress
        """;
    public static async Task<PortRelayUsage[]> Read(CancellationToken token)
    {
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(token);
        timeout.CancelAfter(TimeSpan.FromSeconds(8));
        using var process = new Process { StartInfo = new ProcessStartInfo {
            FileName = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.System), "WindowsPowerShell", "v1.0", "powershell.exe"),
            UseShellExecute = false, CreateNoWindow = true, RedirectStandardOutput = true, RedirectStandardError = true,
        }};
        foreach (var arg in new[] { "-NoProfile", "-NonInteractive", "-EncodedCommand", Convert.ToBase64String(Encoding.Unicode.GetBytes(Query)) }) process.StartInfo.ArgumentList.Add(arg);
        try
        {
            if (!process.Start()) return [];
            var stdout = process.StandardOutput.ReadToEndAsync(timeout.Token);
            var stderr = process.StandardError.ReadToEndAsync(timeout.Token);
            await process.WaitForExitAsync(timeout.Token);
            await stderr;
            if (process.ExitCode != 0) return [];
            return JsonSerializer.Deserialize<PortRelayUsage[]>(await stdout) ?? [];
        }
        catch (Exception ex) when (ex is not OperationCanceledException || !token.IsCancellationRequested) { return []; }
        finally { try { if (!process.HasExited) process.Kill(true); } catch (InvalidOperationException) { } }
    }
    public static string? Blocked(bool storage, bool network, IEnumerable<string> instances, PortRelayUsage[] usage)
    {
        var ids = instances.ToHashSet(StringComparer.OrdinalIgnoreCase);
        foreach (var kind in new[] { "storage", "network" })
        {
            if (!(kind == "storage" ? storage : network)) continue;
            var matches = usage.Where(u => u.Kind == kind && ids.Contains(u.Id)).ToArray();
            if (matches.Length == 0) return $"{kind} usage could not be verified; refresh after the device finishes starting";
            if (matches.Any(m => !m.Safe)) return kind == "storage"
                ? "Take this disk offline in Disk Management before sharing; boot and system disks cannot be shared"
                : "Disable this network adapter in Windows settings before sharing it";
        }
        return null;
    }
}
