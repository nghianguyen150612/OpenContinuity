#include "DeviceDiscoveryReport.h"
#include "JsonValidation.h"
#include <QRegularExpression>
namespace {
bool integer(const QJsonValue &v, int minimum, int maximum) {
  return v.isDouble() && v.toDouble() >= minimum && v.toDouble() <= maximum && v.toDouble() == v.toInt(-1);
}
}
std::optional<DeviceDiscoveryReport> DeviceDiscoveryReport::parse(const QByteArray &bytes) {
  using namespace JsonValidation;
  if (bytes.isEmpty() || bytes.size() > 65536 || !uniqueMembers(bytes)) return {};
  QJsonParseError error;
  const auto doc = QJsonDocument::fromJson(bytes, &error);
  if (error.error != QJsonParseError::NoError || !doc.isObject()) return {};
  const auto r = doc.object();
  if (!keys(r, {"schema_version", "source", "scan_requested", "scan_started", "scan_completed", "requested_duration_seconds", "actual_duration_ms", "adapter_index", "status", "cleanup_status", "devices", "authorization"}) ||
      r.value("schema_version") != QJsonValue(1) || r.value("source") != QJsonValue("bluez_le_discovery_session") ||
      r.value("authorization") != QJsonValue("never_evaluated") || r.value("scan_requested") != QJsonValue(true) ||
      !r.value("scan_started").isBool() || !r.value("scan_completed").isBool() || !r.value("devices").isArray() ||
      !integer(r.value("requested_duration_seconds"), 1, 30) || !integer(r.value("actual_duration_ms"), 0, 40000) ||
      (!r.value("adapter_index").isNull() && !integer(r.value("adapter_index"), 1, 16))) return {};
  const QStringList statuses{"completed", "cancelled", "no_adapter", "adapter_not_powered", "unsupported_le", "adapter_busy", "bluez_unavailable", "system_bus_unavailable", "access_denied", "timeout", "malformed_data", "cleanup_incomplete", "data_limit", "unsupported_platform"};
  DeviceDiscoveryReport result;
  result.status = r.value("status").toString(); result.cleanup = r.value("cleanup_status").toString();
  result.started = r.value("scan_started").toBool(); result.completed = r.value("scan_completed").toBool();
  result.requestedSeconds = r.value("requested_duration_seconds").toInt(); result.actualMs = r.value("actual_duration_ms").toInt();
  if (!statuses.contains(result.status) || !QStringList{"not_needed", "complete", "incomplete"}.contains(result.cleanup)) return {};
  if (result.completed != (result.status == "completed") ||
      (result.completed && (!result.started || result.cleanup != "complete" || result.actualMs < result.requestedSeconds * 1000)) ||
      (result.started && (r.value("adapter_index").isNull() || result.cleanup == "not_needed")) ||
      (result.status == "cleanup_incomplete" && result.cleanup != "incomplete")) return {};
  const auto devices = r.value("devices").toArray();
  if (devices.size() > 128 || (!result.started && (!devices.isEmpty() || result.actualMs != 0))) return {};
  const QRegularExpression idPattern("^scan-[0-9a-f]{16}-[0-9]{3}$");
  const QRegularExpression addressPattern("[0-9A-Fa-f]{2}[:_\\-][0-9A-Fa-f]{2}[:_\\-][0-9A-Fa-f]{2}[:_\\-][0-9A-Fa-f]{2}[:_\\-][0-9A-Fa-f]{2}[:_\\-][0-9A-Fa-f]{2}");
  QString prefix;
  for (qsizetype i = 0; i < devices.size(); ++i) {
    if (!devices[i].isObject()) return {};
    const auto row = devices[i].toObject(); const auto id = row.value("id").toString();
    const auto name = row.value("display_name").toString();
    if (!keys(row, {"id", "display_name", "provenance"}) || !idPattern.match(id).hasMatch() ||
        !id.endsWith(QString("-%1").arg(i + 1, 3, 10, QLatin1Char('0'))) ||
        (!prefix.isEmpty() && !id.startsWith(prefix)) || !safeText(row.value("display_name"), 160) ||
        name.trimmed().isEmpty() || name.toUcs4().size() > 80 || name.contains("/org/bluez/") || addressPattern.match(name).hasMatch() ||
        row.value("provenance") != QJsonValue("bluez_event_during_session")) return {};
    prefix = id.left(22);
    result.devices.append({id, name});
  }
  return result;
}
