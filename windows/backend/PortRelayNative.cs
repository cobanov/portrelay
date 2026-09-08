// SPDX-FileCopyrightText: 2026 Muhammed Cobanov
// SPDX-License-Identifier: GPL-3.0-only
using System.ComponentModel;
using System.IO.Pipes;
using System.Net;
using System.Runtime.InteropServices;
using System.Security.Principal;
using Microsoft.Win32.SafeHandles;

namespace Usbipd;

static class PortRelayNative
{
    [StructLayout(LayoutKind.Sequential)]
    struct SecurityAttributes { public int Length; public IntPtr Descriptor; public int Inherit; }
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool ConvertStringSecurityDescriptorToSecurityDescriptor(string text, uint revision, out IntPtr descriptor, out uint size);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern SafePipeHandle CreateNamedPipe(string name, uint mode, uint pipeMode, uint instances, uint output, uint input, uint timeout, ref SecurityAttributes attributes);
    [DllImport("kernel32.dll")]
    static extern IntPtr LocalFree(IntPtr memory);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetNamedPipeClientProcessId(SafePipeHandle pipe, out uint pid);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool GetNamedPipeServerProcessId(SafePipeHandle pipe, out uint pid);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern SafeProcessHandle OpenProcess(uint access, bool inherit, uint pid);
    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool OpenProcessToken(SafeProcessHandle process, uint access, out SafeAccessTokenHandle token);
    [DllImport("iphlpapi.dll", SetLastError = true)]
    static extern uint GetExtendedTcpTable(IntPtr table, ref uint size, bool order, uint family, uint tableClass, uint reserved);

    public static NamedPipeServerStream CreatePipe(string name, string? owner, bool first = false)
    {
        var sddl = "D:P(A;;GA;;;SY)" + (owner is null ? "" : $"(A;;GRGW;;;{new SecurityIdentifier(owner).Value})");
        if (!ConvertStringSecurityDescriptorToSecurityDescriptor(sddl, 1, out var descriptor, out _))
            throw new Win32Exception();
        try
        {
            var attributes = new SecurityAttributes { Length = Marshal.SizeOf<SecurityAttributes>(), Descriptor = descriptor };
            // Duplex, overlapped, byte mode, PIPE_REJECT_REMOTE_CLIENTS. No SMB access.
            var handle = CreateNamedPipe(@"\\.\pipe\" + name, 3 | 0x40000000u | (first ? 0x80000u : 0), 8, 32, 65536, 65536, 0, ref attributes);
            if (handle.IsInvalid) { handle.Dispose(); throw new Win32Exception(); }
            return new NamedPipeServerStream(PipeDirection.InOut, true, false, handle);
        }
        finally { _ = LocalFree(descriptor); }
    }

    static bool ProcessIs(uint pid, string sid)
    {
        using var process = OpenProcess(0x1000, false, pid);
        if (process.IsInvalid || !OpenProcessToken(process, 8, out var token)) return false;
        using (token)
        using (var identity = new WindowsIdentity(token.DangerousGetHandle()))
            return identity.User?.Value == sid;
    }
    public static bool ClientIs(NamedPipeServerStream pipe, string sid) =>
        GetNamedPipeClientProcessId(pipe.SafePipeHandle, out var pid) && ProcessIs(pid, sid);
    public static bool ServerIsSystem(NamedPipeClientStream pipe) =>
        GetNamedPipeServerProcessId(pipe.SafePipeHandle, out var pid) && ProcessIs(pid, "S-1-5-18");

    // Verify the *client* side of the exact accepted TCP tuple. A loopback address
    // by itself is not authorization: ordinary local processes must be rejected.
    public static bool IsKernelConnection(IPEndPoint client, IPEndPoint server)
    {
        if (!client.Address.Equals(IPAddress.Loopback) || !server.Address.Equals(IPAddress.Loopback)) return false;
        uint size = 0;
        if (GetExtendedTcpTable(IntPtr.Zero, ref size, false, 2, 5, 0) != 122 || size > 16 * 1024 * 1024) return false;
        var buffer = Marshal.AllocHGlobal(checked((int)size));
        try
        {
            if (GetExtendedTcpTable(buffer, ref size, false, 2, 5, 0) != 0) return false;
            var count = Marshal.ReadInt32(buffer);
            if (count < 0 || 4L + count * 24L > size) return false;
            for (var i = 0; i < count; i++)
            {
                var row = IntPtr.Add(buffer, 4 + i * 24);
                var localPort = (ushort)IPAddress.NetworkToHostOrder((short)Marshal.ReadInt32(row, 8));
                var remotePort = (ushort)IPAddress.NetworkToHostOrder((short)Marshal.ReadInt32(row, 16));
                if (Marshal.ReadInt32(row, 4) == 0x0100007f && Marshal.ReadInt32(row, 12) == 0x0100007f
                    && localPort == client.Port && remotePort == server.Port && Marshal.ReadInt32(row, 20) == 4) return true;
            }
            return false;
        }
        finally { Marshal.FreeHGlobal(buffer); }
    }
}
