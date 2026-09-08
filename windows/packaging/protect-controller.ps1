$ErrorActionPreference='Stop'
# Upstream permits ordinary users to open the virtual controller. PortRelay
# owns a dedicated installation and restricts its control device to SYSTEM and
# administrators, so another user cannot ask the kernel to claim our USB stream.
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
public static class PortRelayControllerSecurity {
 [StructLayout(LayoutKind.Sequential)] struct DeviceInfo { public uint Size; public Guid Class; public uint Instance; public UIntPtr Reserved; }
 [DllImport("setupapi.dll", SetLastError=true)] static extern IntPtr SetupDiCreateDeviceInfoList(IntPtr guid, IntPtr parent);
 [DllImport("setupapi.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool SetupDiOpenDeviceInfo(IntPtr set, string id, IntPtr parent, uint flags, ref DeviceInfo info);
 [DllImport("setupapi.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool SetupDiGetDeviceRegistryProperty(IntPtr set, ref DeviceInfo info, uint property, out uint type, byte[] value, uint size, out uint needed);
 [DllImport("setupapi.dll", CharSet=CharSet.Unicode, SetLastError=true)] static extern bool SetupDiSetDeviceRegistryProperty(IntPtr set, ref DeviceInfo info, uint property, byte[] value, uint size);
 [DllImport("setupapi.dll")] static extern bool SetupDiDestroyDeviceInfoList(IntPtr set);
 public static bool Restrict(string id) {
  var set=SetupDiCreateDeviceInfoList(IntPtr.Zero,IntPtr.Zero);
  if(set==new IntPtr(-1))throw new Win32Exception();
  try {
   var info=new DeviceInfo {Size=(uint)Marshal.SizeOf(typeof(DeviceInfo))};
   if(!SetupDiOpenDeviceInfo(set,id,IntPtr.Zero,0,ref info))throw new Win32Exception();
   const string desired="D:P(A;;GA;;;SY)(A;;GA;;;BA)";
   var buffer=new byte[16384]; uint type,needed;
   if(SetupDiGetDeviceRegistryProperty(set,ref info,0x18,out type,buffer,(uint)buffer.Length,out needed)) {
    if(Encoding.Unicode.GetString(buffer,0,(int)needed).TrimEnd('\0')==desired)return false;
   } else if(Marshal.GetLastWin32Error()!=13)throw new Win32Exception();
   buffer=Encoding.Unicode.GetBytes(desired+"\0");
   if(!SetupDiSetDeviceRegistryProperty(set,ref info,0x18,buffer,(uint)buffer.Length))throw new Win32Exception();
   return true;
  } finally {SetupDiDestroyDeviceInfoList(set);}
 }
}
'@
$controllers=@(Get-CimInstance Win32_PnPEntity | Where-Object {$_.Service -eq 'usbip2_ude'})
if($controllers.Count -ne 1){throw 'Restart Windows to finish installing the USB controller, then enable USB support again.'}
if([PortRelayControllerSecurity]::Restrict($controllers[0].PNPDeviceID)) {
    & "$env:SystemRoot\System32\pnputil.exe" /restart-device $controllers[0].PNPDeviceID
    if($LASTEXITCODE -ne 0){throw 'Restart Windows to finish securing the USB controller, then enable USB support again.'}
}
