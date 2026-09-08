#!/usr/bin/env python3
"""Apply the PortRelay GPL backend additions to exactly usbipd-win v5.3.0."""
import pathlib
import shutil
import subprocess
import sys

root = pathlib.Path(sys.argv[1]).resolve()
expected = "aa3db8b82c4cb5071fd31bc54211606c70886912"
actual = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
if actual != expected:
    raise SystemExit("Unexpected usbipd-win revision")


def replace(name, old, new):
    path = root / "Usbipd" / name
    text = path.read_text(encoding="utf-8-sig")
    if old not in text:
        raise SystemExit(f"Patch does not apply: {name}")
    path.write_text(text.replace(old, new), encoding="utf-8")


replace("ClientContext.cs", "public TcpClient TcpClient { get; set; } = new();", "public Stream TcpClient { get; set; } = Stream.Null;")
for name in ["ConnectedClient.cs", "AttachedClient.cs", "AttachedEndpoint.cs"]:
    path = root / "Usbipd" / name
    text = path.read_text(encoding="utf-8-sig")
    text = text.replace("NetworkStream", "Stream").replace("TcpClient tcpClient", "Stream tcpClient")
    text = text.replace("clientContext.TcpClient.GetStream()", "clientContext.TcpClient")
    text = text.replace("tcpClient.GetStream()", "tcpClient").replace("tcpClient.NoDelay = true;", "// Named pipes do not have Nagle buffering.")
    path.write_text(text, encoding="utf-8")

path = root / "Usbipd" / "Server.cs"
text = path.read_text(encoding="utf-8-sig")
start = text.index('        if (!ushort.TryParse(Configuration["usbipd:Port"]')
end = text.index("\n    }", start)
text = text[:start] + text[end:]
text = text.replace("    readonly TcpListener TcpListener;", "")
text = text.replace("        TcpListener.Dispose();", "")
start = text.index("        // All client sockets will inherit these.")
end = text.index("            _ = Task.Run", start)
text = text[:start] + '''        if (!System.Security.Principal.WindowsIdentity.GetCurrent().IsSystem)
            throw new UnauthorizedAccessException("PortRelay's USB service requires LocalSystem.");
        while (!stoppingToken.IsCancellationRequested)
        {
            var tcpClient = PortRelayNative.CreatePipe("PortRelay.UsbIp.v1", null);
            try { await tcpClient.WaitForConnectionAsync(stoppingToken); }
            catch { tcpClient.Dispose(); throw; }
            if (!PortRelayNative.ClientIs(tcpClient, "S-1-5-18"))
            { tcpClient.Dispose(); continue; }
            var clientAddress = IPAddress.Loopback;

''' + text[end:]
path.write_text(text, encoding="utf-8")
replace("CommandHandlersServer.cs", ".UseWindowsService()", '.UseWindowsService(options => options.ServiceName = "PortRelayHelper")')
replace("CommandHandlersServer.cs", "_ = services.AddHostedService<Server>();", "_ = services.AddHostedService<Server>();\n                _ = services.AddHostedService<PortRelayService>();")
replace("Usbipd.csproj", "<PublishAot>true</PublishAot>", "<PublishAot>false</PublishAot>\n    <PublishTrimmed>false</PublishTrimmed>\n    <IsTrimmable>false</IsTrimmable>\n    <IsAotCompatible>false</IsAotCompatible>\n    <JsonSerializerIsReflectionEnabledByDefault>true</JsonSerializerIsReflectionEnabledByDefault>")
for source in pathlib.Path(__file__).parent.glob("PortRelay*.cs"):
    shutil.copy2(source, root / "Usbipd" / source.name)
path = root / "Usbipd" / "NativeMethods.txt"
with path.open("a", encoding="utf-8") as stream:
    stream.write("\nDEVPKEY_Device_LastArrivalDate\nDEVPKEY_Device_Class\nDEVPKEY_Device_Service\nDEVPKEY_Device_Capabilities\n")

shutil.copy2(pathlib.Path(__file__).parent / "packages.lock.json", root / "Usbipd" / "packages.lock.json")
import json
path = root / "global.json"
config = json.loads(path.read_text())
config["sdk"] = {"version": "9.0.317", "allowPrerelease": False, "rollForward": "disable"}
path.write_text(json.dumps(config, indent=2) + "\n")
