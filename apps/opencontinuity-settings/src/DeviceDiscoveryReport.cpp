#include "DeviceDiscoveryReport.h"
#include "JsonValidation.h"
#include <QJsonParseError>
#include <QRegularExpression>

namespace {
constexpr int kMaxRecordBytes = 2 * 1024;
constexpr int kMaxDeviceEvents = 512;
constexpr int kMaxDevices = 128;

bool integer(const QJsonValue &value, int minimum, int maximum) {
  return value.isDouble() && value.toDouble() >= minimum &&
         value.toDouble() <= maximum && value.toDouble() == value.toInt(-1);
}

QStringList statuses() {
  return {"completed", "cancelled", "no_adapter", "adapter_not_powered",
          "unsupported_le", "adapter_busy", "bluez_unavailable",
          "system_bus_unavailable", "access_denied", "timeout",
          "malformed_data", "cleanup_incomplete", "data_limit",
          "output_blocked", "output_closed", "unsupported_platform"};
}

QString reasonFor(const QString &status) {
  if (status == "completed") return "duration_elapsed";
  if (status == "cancelled") return "user_cancelled";
  if (status == "no_adapter") return "adapter_lost_or_unavailable";
  if (status == "adapter_not_powered") return "adapter_power_lost_or_off";
  if (status == "bluez_unavailable") return "bluez_owner_lost_or_unavailable";
  if (status == "timeout") return "deadline_expired";
  if (status == "data_limit") return "stream_or_input_limit";
  if (status == "output_blocked") return "stdout_blocked";
  if (status == "output_closed") return "stdout_closed";
  if (status == "cleanup_incomplete") return "cleanup_failed";
  return "setup_or_observation_failed";
}

bool common(const QJsonObject &object, const QByteArray &bytes, int sequence,
            QString &sessionId, QString *error) {
  using namespace JsonValidation;
  static const QRegularExpression sessionPattern("^[0-9a-f]{32}$");
  if (bytes.size() > kMaxRecordBytes || !uniqueMembers(bytes)) {
    if (error) *error = "Event exceeded its record limit or contains duplicate fields.";
    return false;
  }
  if (object.value("schema_version") != QJsonValue(1) ||
      object.value("source") != QJsonValue("bluez_le_discovery_events") ||
      object.value("authorization") != QJsonValue("never_evaluated") ||
      !integer(object.value("sequence"), 1, 514) ||
      object.value("sequence").toInt() != sequence ||
      !object.value("scan_session_id").isString()) {
    if (error) *error = "Event schema, source, authorization, or sequence is invalid.";
    return false;
  }
  const auto currentId = object.value("scan_session_id").toString();
  if (!sessionPattern.match(currentId).hasMatch() ||
      (!sessionId.isEmpty() && currentId != sessionId)) {
    if (error) *error = "Event belongs to an invalid or different scan session.";
    return false;
  }
  if (sessionId.isEmpty()) sessionId = currentId;
  return true;
}

bool adapterIndex(const QJsonValue &value, int &result) {
  if (value.isNull()) { result = 0; return true; }
  if (!integer(value, 1, 16)) return false;
  result = value.toInt();
  return true;
}
}

void DeviceDiscoveryReport::reset() {
  status.clear(); cleanup.clear(); sessionId.clear(); sessionStatus.clear();
  started = false; completed = false; sessionSeen = false; terminalSeen = false;
  requestedSeconds = 0; actualMs = 0; deviceEventCount = 0;
  m_nextSequence = 1; m_adapterIndex = 0; m_ids.clear();
}

bool DeviceDiscoveryReport::consumeLine(const QByteArray &line,
                                        DeviceDiscoveryModel &model,
                                        QString *error) {
  if (error) error->clear();
  if (line.isEmpty() || line.size() > kMaxRecordBytes) {
    if (error) *error = "Event line is empty or too large.";
    return false;
  }
  QJsonParseError parseError;
  const auto document = QJsonDocument::fromJson(line, &parseError);
  if (parseError.error != QJsonParseError::NoError || !document.isObject()) {
    if (error) *error = "Event line is not a UTF-8 JSON object.";
    return false;
  }
  const auto object = document.object();
  const auto type = object.value("type").toString();
  if (terminalSeen || (!sessionSeen && type != "session")) {
    if (error) *error = "Event ordering is invalid.";
    return false;
  }
  QString candidateSession = sessionId;
  if (!common(object, line, m_nextSequence, candidateSession, error)) return false;
  const auto nextSequence = m_nextSequence + 1;

  if (type == "session") {
    using namespace JsonValidation;
    if (sessionSeen || !keys(object, {"schema_version", "source", "type",
                                      "authorization", "scan_session_id", "sequence",
                                      "state", "status", "scan_started",
                                      "requested_duration_seconds", "adapter_index"}) ||
        !integer(object.value("requested_duration_seconds"), 1, 30) ||
        !object.value("scan_started").isBool() ||
        !adapterIndex(object.value("adapter_index"), m_adapterIndex)) {
      if (error) *error = "Session event fields are invalid.";
      return false;
    }
    started = object.value("scan_started").toBool();
    const auto state = object.value("state").toString();
    sessionStatus = object.value("status").toString();
    requestedSeconds = object.value("requested_duration_seconds").toInt();
    if ((started && (state != "started" || sessionStatus != "started" || m_adapterIndex == 0)) ||
        (!started && (state != "not_started" || !statuses().contains(sessionStatus)))) {
      if (error) *error = "Session state does not match its start status.";
      return false;
    }
    sessionSeen = true;
  } else if (type == "device") {
    using namespace JsonValidation;
    static const QRegularExpression addressPattern(
        "[0-9A-Fa-f]{2}[:_\\-][0-9A-Fa-f]{2}[:_\\-][0-9A-Fa-f]{2}[:_\\-][0-9A-Fa-f]{2}[:_\\-][0-9A-Fa-f]{2}[:_\\-][0-9A-Fa-f]{2}");
    const QStringList fields{"schema_version", "source", "type", "authorization",
                             "scan_session_id", "sequence", "id", "display_name",
                             "provenance", "change"};
    const auto id = object.value("id").toString();
    const auto name = object.value("display_name").toString();
    const auto change = object.value("change").toString();
    const auto expectedPrefix = "scan-" + sessionId.left(16) + "-";
    static const QRegularExpression idPattern("^scan-[0-9a-f]{16}-[0-9]{3}$");
    if (!keys(object, fields) || !started || deviceEventCount >= kMaxDeviceEvents ||
        !idPattern.match(id).hasMatch() || !id.startsWith(expectedPrefix) ||
        !safeText(object.value("display_name"), 320) || name.trimmed().isEmpty() ||
        name.toUcs4().size() > 80 || name.contains("/org/bluez/") ||
        addressPattern.match(name).hasMatch() ||
        object.value("provenance") != QJsonValue("bluez_event_during_session") ||
        (change != "observed" && change != "updated")) {
      if (error) *error = "Device event contains invalid or unsafe fields.";
      return false;
    }
    if (change == "observed") {
      const auto expectedId = expectedPrefix + QString("%1").arg(m_ids.size() + 1, 3, 10, QLatin1Char('0'));
      if (m_ids.contains(id) || id != expectedId || m_ids.size() >= kMaxDevices) {
        if (error) *error = "New device identifier is duplicate or out of order.";
        return false;
      }
      m_ids.insert(id);
    } else if (!m_ids.contains(id) || model.displayName(id) == name) {
      if (error) *error = "Device update does not refer to a changed existing observation.";
      return false;
    }
    model.upsert({id, name});
    ++deviceEventCount;
  } else if (type == "terminal") {
    using namespace JsonValidation;
    int terminalAdapter = 0;
    if (!keys(object, {"schema_version", "source", "type", "authorization",
                       "scan_session_id", "sequence", "scan_requested", "scan_started",
                       "scan_completed", "requested_duration_seconds", "actual_duration_ms",
                       "adapter_index", "status", "cleanup_status", "device_count",
                       "device_event_count", "termination_reason"}) ||
        object.value("scan_requested") != QJsonValue(true) ||
        !object.value("scan_started").isBool() || !object.value("scan_completed").isBool() ||
        !integer(object.value("requested_duration_seconds"), 1, 30) ||
        !integer(object.value("actual_duration_ms"), 0, 40000) ||
        !adapterIndex(object.value("adapter_index"), terminalAdapter) ||
        !integer(object.value("device_count"), 0, kMaxDevices) ||
        !integer(object.value("device_event_count"), 0, kMaxDeviceEvents)) {
      if (error) *error = "Terminal event fields are invalid.";
      return false;
    }
    status = object.value("status").toString();
    cleanup = object.value("cleanup_status").toString();
    const bool terminalStarted = object.value("scan_started").toBool();
    completed = object.value("scan_completed").toBool();
    const int terminalRequestedSeconds = object.value("requested_duration_seconds").toInt();
    const int terminalActualMs = object.value("actual_duration_ms").toInt();
    const int terminalDeviceCount = object.value("device_count").toInt();
    const int terminalEventCount = object.value("device_event_count").toInt();
    actualMs = terminalActualMs;
    if (!statuses().contains(status) ||
        !QStringList{"not_needed", "complete", "incomplete"}.contains(cleanup) ||
        terminalStarted != started || terminalAdapter != m_adapterIndex ||
        requestedSeconds != terminalRequestedSeconds ||
        terminalDeviceCount != model.count() || terminalDeviceCount != m_ids.size() ||
        terminalEventCount != deviceEventCount ||
        object.value("termination_reason").toString() != reasonFor(status) ||
        completed != (status == "completed" && cleanup == "complete") ||
        (status == "completed" && (!started || cleanup != "complete" || terminalActualMs < requestedSeconds * 1000)) ||
        (!started && (!m_ids.isEmpty() || terminalActualMs != 0)) ||
        (status == "cleanup_incomplete" && cleanup != "incomplete") ||
        (started && cleanup == "not_needed") ||
        (!started && sessionStatus != status)) {
      if (error) *error = "Terminal status, counts, or cleanup do not match the stream.";
      return false;
    }
    terminalSeen = true;
  } else {
    if (error) *error = "Unknown event category.";
    return false;
  }
  sessionId = candidateSession;
  m_nextSequence = nextSequence;
  return true;
}

bool DeviceDiscoveryReport::finish(int exitCode, bool normalExit,
                                   const DeviceDiscoveryModel &model,
                                   QString *error) const {
  if (error) error->clear();
  const bool valid = normalExit && sessionSeen && terminalSeen &&
                     model.count() == m_ids.size() &&
                     exitCode == (status == "completed" ? 0 : 1);
  if (!valid && error) *error = "CLI exit did not match a complete validated JSONL stream.";
  return valid;
}
