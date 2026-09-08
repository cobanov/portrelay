// SPDX-FileCopyrightText: 2026 Muhammed Cobanov
// SPDX-License-Identifier: GPL-3.0-only
namespace Usbipd;

sealed record PortRelayDevice(string Id, string Generation, string Name, string Vendor, string Product,
    string Kind, uint Speed, uint Devid, string? Blocked, [property: System.Text.Json.Serialization.JsonIgnore(Condition = System.Text.Json.Serialization.JsonIgnoreCondition.WhenWritingNull)] string[]? Risks = null,
    string? ParentHub = null);
sealed record PortRelayEntry(PortRelayDevice Device, string InstanceId, bool UniqueIdentity, string Fingerprint, string RawGeneration);
