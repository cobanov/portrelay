// SPDX-FileCopyrightText: 2026 Muhammed Cobanov
// SPDX-License-Identifier: GPL-3.0-only
using System.Buffers.Binary;
using System.Text;
using System.Text.Json;

namespace Usbipd;

static class PortRelayWire
{
    public static readonly JsonSerializerOptions Json = new() { PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower };
    public static async Task<JsonDocument> Read(Stream stream, CancellationToken token)
    {
        var header = new byte[4];
        await stream.ReadExactlyAsync(header, token);
        var size = BinaryPrimitives.ReadUInt32BigEndian(header);
        if (size == 0 || size > 65536) throw new InvalidDataException("Invalid frame length");
        var bytes = new byte[size];
        await stream.ReadExactlyAsync(bytes, token);
        return JsonDocument.Parse(bytes, new JsonDocumentOptions { MaxDepth = 16 });
    }
    public static async Task Write(Stream stream, object value, CancellationToken token)
    {
        var bytes = JsonSerializer.SerializeToUtf8Bytes(value, Json);
        if (bytes.Length > 65536) throw new InvalidDataException("Reply too large");
        var header = new byte[4];
        BinaryPrimitives.WriteUInt32BigEndian(header, (uint)bytes.Length);
        await stream.WriteAsync(header, token);
        await stream.WriteAsync(bytes, token);
        await stream.FlushAsync(token);
    }
    public static object Reply(string? error = null, PortRelayDevice? device = null, uint? port = null) => new { error, device, port };
    public static bool ValidBus(string bus) => bus.Length is > 2 and < 32
        && System.Text.RegularExpressions.Regex.IsMatch(bus, @"^[0-9]+-[0-9]+(?:\.[0-9]+)*$", System.Text.RegularExpressions.RegexOptions.CultureInvariant);
    public static void Validate(PortRelayDevice device)
    {
        if (!ValidBus(device.Id) || !new uint[] { 1, 2, 3, 5, 6 }.Contains(device.Speed)
            || device.Devid >> 16 == 0 || (device.Devid & 65535) == 0
            || device.Vendor.Length != 4 || device.Product.Length != 4
            || !ushort.TryParse(device.Vendor, System.Globalization.NumberStyles.HexNumber, null, out _)
            || !ushort.TryParse(device.Product, System.Globalization.NumberStyles.HexNumber, null, out _))
            throw new InvalidDataException("Invalid USB device metadata");
    }
    public static async Task<byte[]> Import(Stream stream, string bus, CancellationToken token)
    {
        if (!ValidBus(bus)) throw new InvalidDataException("Invalid USB bus ID");
        var request = new byte[40];
        request[0] = 1; request[1] = 0x11; request[2] = 0x80; request[3] = 3;
        Encoding.ASCII.GetBytes(bus).CopyTo(request, 8);
        await stream.WriteAsync(request, token);
        var response = new byte[8];
        await stream.ReadExactlyAsync(response, token);
        if (!response.AsSpan().SequenceEqual(new byte[] { 1, 0x11, 0, 3, 0, 0, 0, 0 }))
            throw new IOException("The Windows USB driver could not claim this device");
        var descriptor = new byte[312];
        await stream.ReadExactlyAsync(descriptor, token);
        if (Encoding.ASCII.GetString(descriptor, 256, 32).TrimEnd('\0') != bus)
            throw new InvalidDataException("USB identity changed during attachment");
        return descriptor;
    }
    public static async Task AcceptImport(Stream stream, PortRelayDevice device, CancellationToken token)
    {
        Validate(device);
        var request = new byte[40];
        await stream.ReadExactlyAsync(request, token);
        if (!request.AsSpan(0, 8).SequenceEqual(new byte[] { 1, 0x11, 0x80, 3, 0, 0, 0, 0 })
            || Encoding.ASCII.GetString(request, 8, 32).TrimEnd('\0') != device.Id)
            throw new InvalidDataException("Unexpected USB/IP import request");
        var response = new byte[320];
        response[0] = 1; response[1] = 0x11; response[3] = 3;
        Encoding.ASCII.GetBytes("PortRelay").CopyTo(response, 8);
        Encoding.ASCII.GetBytes(device.Id).CopyTo(response, 264);
        BinaryPrimitives.WriteUInt32BigEndian(response.AsSpan(296), device.Devid >> 16);
        BinaryPrimitives.WriteUInt32BigEndian(response.AsSpan(300), device.Devid & 65535);
        BinaryPrimitives.WriteUInt32BigEndian(response.AsSpan(304), device.Speed);
        BinaryPrimitives.WriteUInt16BigEndian(response.AsSpan(308), Convert.ToUInt16(device.Vendor, 16));
        BinaryPrimitives.WriteUInt16BigEndian(response.AsSpan(310), Convert.ToUInt16(device.Product, 16));
        // The UDE driver fetches the actual device/configuration descriptors as URBs.
        await stream.WriteAsync(response, token);
        await stream.FlushAsync(token);
    }
    public static async Task Bridge(Stream left, Stream right, CancellationToken token)
    {
        using var cancel = CancellationTokenSource.CreateLinkedTokenSource(token);
        var a = left.CopyToAsync(right, 65536, cancel.Token);
        var b = right.CopyToAsync(left, 65536, cancel.Token);
        var completed = await Task.WhenAny(a, b);
        await cancel.CancelAsync();
        // EOF in either direction ends the lease. Close both ends before recovery.
        left.Dispose(); right.Dispose();
        try { await Task.WhenAll(a, b); } catch (Exception) when (completed.IsCompletedSuccessfully || token.IsCancellationRequested) { }
    }
}
