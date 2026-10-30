#include "DoctorReport.h"
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSet>

namespace {
// QJsonDocument accepts duplicate object members. Reject them before trusting
// its normalized object; decode escaped keys so equivalent spellings collide.
bool uniqueMembers(const QByteArray &bytes) {
  QVector<QSet<QString>> objects;
  for (qsizetype i = 0; i < bytes.size(); ++i) {
    const char c = bytes[i];
    if (c == '{') {
      objects.append(QSet<QString>{});
      if (objects.size() > 8)
        return false;
    } else if (c == '}')
      objects.removeLast();
    else if (c == '"') {
      const auto start = i++;
      for (; i < bytes.size(); ++i) {
        if (bytes[i] == '\\')
          ++i;
        else if (bytes[i] == '"')
          break;
      }
      auto next = i + 1;
      while (next < bytes.size() &&
             (bytes[next] == ' ' || bytes[next] == '\n' ||
              bytes[next] == '\r' || bytes[next] == '\t'))
        ++next;
      if (next < bytes.size() && bytes[next] == ':') {
        const auto key =
            QJsonDocument::fromJson("[" + bytes.mid(start, i - start + 1) + "]")
                .array()[0]
                .toString();
        if (objects.isEmpty() || objects.last().contains(key))
          return false;
        objects.last().insert(key);
      }
    }
  }
  return true;
}

bool keys(const QJsonObject &o, const QStringList &expected) {
  const auto actual = o.keys();
  return QSet<QString>(actual.cbegin(), actual.cend()) ==
         QSet<QString>(expected.cbegin(), expected.cend());
}
bool text(const QJsonValue &v, int maximum) {
  if (!v.isString() || v.toString().isEmpty() || v.toString().size() > maximum)
    return false;
  for (const auto c : v.toString())
    if (c.category() == QChar::Other_Control ||
        c.category() == QChar::Other_Format)
      return false;
  return true;
}
bool state(const QJsonValue &v) {
  static const QStringList states{"supported", "experimental", "unsupported",
                                  "blocked", "unknown"};
  return v.isString() && states.contains(v.toString());
}
} // namespace
std::optional<DoctorReport> DoctorReport::parse(const QByteArray &bytes) {
  if (bytes.isEmpty() || bytes.size() > MaxOutput)
    return {};
  QJsonParseError error;
  const auto document = QJsonDocument::fromJson(bytes, &error);
  if (error.error != QJsonParseError::NoError || !document.isObject() ||
      !uniqueMembers(bytes))
    return {};
  const auto o = document.object();
  if (!keys(o, {"schema_version", "project", "version", "capabilities",
                "watch_auto_unlock_trust"}) ||
      !o["schema_version"].isDouble() || o["schema_version"].toDouble() != 1 ||
      o["project"] != QJsonValue("OpenContinuity") || !text(o["version"], 64) ||
      !o["capabilities"].isArray() || !o["watch_auto_unlock_trust"].isObject())
    return {};
  const QStringList ids{"apple_watch_auto_unlock", "airdrop",
                        "airpods_settings"};
  const QStringList titles{"Apple Watch Auto Unlock", "AirDrop",
                           "AirPods settings"};
  const auto capabilities = o["capabilities"].toArray();
  if (capabilities.size() != ids.size())
    return {};
  DoctorReport report;
  report.version = o["version"].toString();
  QSet<QString> seen;
  for (const auto &value : capabilities) {
    if (!value.isObject())
      return {};
    const auto row = value.toObject();
    const auto id = row["id"].toString();
    const int index = ids.indexOf(id);
    if (!keys(row, {"id", "state", "summary"}) || index < 0 ||
        seen.contains(id) || !state(row["state"]) || !text(row["summary"], 512))
      return {};
    seen.insert(id);
    report.capabilities.append({id, titles[index], row["state"].toString(),
                                row["summary"].toString()});
  }
  const QStringList stages{"device_detection",
                           "trusted_identity",
                           "cryptographic_authentication",
                           "session_freshness",
                           "proximity_assurance",
                           "operational_readiness"};
  const QStringList labels{"Device detection",
                           "Trusted identity",
                           "Cryptographic authentication",
                           "Session freshness",
                           "Proximity assurance",
                           "Operational readiness"};
  const auto trust = o["watch_auto_unlock_trust"].toObject();
  if (!keys(trust, stages))
    return {};
  for (int i = 0; i < stages.size(); ++i) {
    if (!state(trust[stages[i]]))
      return {};
    report.trust.append(
        {stages[i], labels[i], trust[stages[i]].toString(), {}});
  }
  return report;
}
