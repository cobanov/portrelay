// SPDX-FileCopyrightText: 2026 Muhammed Cobanov
// SPDX-License-Identifier: GPL-3.0-only
using System;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.AccessControl;
using Microsoft.Win32.SafeHandles;

namespace Usbipd
{
    public static class PortRelayController
    {
        [DllImport("cfgmgr32.dll", CharSet = CharSet.Unicode)]
        static extern uint CM_Get_Device_Interface_List_Size(out uint size, ref Guid guid, string instance, uint flags);
        [DllImport("cfgmgr32.dll", CharSet = CharSet.Unicode)]
        static extern uint CM_Get_Device_Interface_List(ref Guid guid, string instance, IntPtr buffer, uint size, uint flags);
        [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
        static extern SafeFileHandle CreateFile(string path, uint access, uint share, IntPtr attributes, uint creation, uint flags, IntPtr template);
        [DllImport("advapi32.dll")]
        static extern uint GetSecurityInfo(SafeFileHandle handle, uint type, uint information, out IntPtr owner, out IntPtr group, out IntPtr dacl, out IntPtr sacl, out IntPtr descriptor);
        [DllImport("advapi32.dll")]
        static extern uint GetSecurityDescriptorLength(IntPtr descriptor);
        [DllImport("kernel32.dll")]
        static extern IntPtr LocalFree(IntPtr memory);

        public static void RequirePrivate()
        {
            var guid = new Guid("b4030c06-dc5f-4fcc-87eb-e5515a0935c0");
            uint size;
            if (CM_Get_Device_Interface_List_Size(out size, ref guid, null, 0) != 0 || size < 2 || size > 65536)
                throw new IOException("The Windows USB controller is unavailable. Restart Windows and enable USB support.");
            var buffer = Marshal.AllocHGlobal(checked((int)size * 2));
            string path;
            try
            {
                if (CM_Get_Device_Interface_List(ref guid, null, buffer, size, 0) != 0) throw new IOException("Cannot find the Windows USB controller");
                var paths = Marshal.PtrToStringUni(buffer, (int)size).Split(new[] { '\0' }, StringSplitOptions.RemoveEmptyEntries);
                if (paths.Length != 1) throw new IOException("PortRelay requires one dedicated USB controller");
                path = paths[0];
            }
            finally { Marshal.FreeHGlobal(buffer); }
            using (var handle = CreateFile(path, 0x20000, 3, IntPtr.Zero, 3, 0, IntPtr.Zero))
            {
                if (handle.IsInvalid) throw new Win32Exception();
                IntPtr owner, group, dacl, sacl, descriptor;
                var error = GetSecurityInfo(handle, 1, 5, out owner, out group, out dacl, out sacl, out descriptor);
                if (error != 0) throw new Win32Exception((int)error);
                RawSecurityDescriptor security;
                try
                {
                    var length = GetSecurityDescriptorLength(descriptor);
                    if (length == 0 || length > 65536) throw new IOException("Invalid USB controller security");
                    var bytes = new byte[length];
                    Marshal.Copy(descriptor, bytes, 0, bytes.Length);
                    security = new RawSecurityDescriptor(bytes, 0);
                }
                finally { LocalFree(descriptor); }
                if (security.DiscretionaryAcl == null || security.DiscretionaryAcl.Count == 0
                    || (security.Owner.Value != "S-1-5-18" && security.Owner.Value != "S-1-5-32-544"))
                    throw new IOException("Enable USB support again to secure the Windows controller");
                foreach (GenericAce entry in security.DiscretionaryAcl)
                {
                    var ace = entry as CommonAce;
                    if (ace == null || ace.IsCallback || ace.AceQualifier != AceQualifier.AccessAllowed
                        || (ace.SecurityIdentifier.Value != "S-1-5-18" && ace.SecurityIdentifier.Value != "S-1-5-32-544"))
                        throw new IOException("Enable USB support again to restrict the Windows controller to the device service");
                }
            }
        }
    }
}
