// SPDX-FileCopyrightText: 2026 Muhammed Cobanov
// SPDX-License-Identifier: GPL-3.0-only
namespace Usbipd;

sealed record PortRelayDevice(string Id, string Generation, string Name, string Vendor, string Product,
    string Kind, uint Speed, uint Devid, string? Blocked);
sealed record PortRelayEntry(PortRelayDevice Device, string InstanceId);

