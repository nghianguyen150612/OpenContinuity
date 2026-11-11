#include "AirdropStatusReport.h"
#include "JsonValidation.h"
#include <QJsonParseError>
#include <QRegularExpression>

namespace {
constexpr qsizetype kMaxReportBytes = 64 * 1024;

bool oneOf(const QJsonValue &value, const QStringList &allowed) {
  return value.isString() && allowed.contains(value.toString());
}

bool integer(const QJsonValue &value, int minimum, int maximum) {
  return value.isDouble() && value.toDouble() >= minimum &&
         value.toDouble() <= maximum && value.toDouble() == value.toInt(-1);
}

bool optionalText(const QJsonValue &value, const QRegularExpression &pattern,
                  QString &target) {
  if (value.isNull()) {
    target = "unknown";
    return true;
  }
  if (!value.isString() || !pattern.match(value.toString()).hasMatch())
    return false;
  target = value.toString();
  return true;
}

const QStringList &states() {
  static const QStringList values{"unsupported", "unknown", "available",
                                 "blocked", "ready", "active", "failed"};
  return values;
}
} // namespace

std::optional<AirdropStatusReport> AirdropStatusReport::parse(
    const QByteArray &bytes) {
  using namespace JsonValidation;
  if (bytes.isEmpty() || bytes.size() > kMaxReportBytes ||
      !uniqueMembers(bytes))
    return std::nullopt;
  QJsonParseError error;
  const auto document = QJsonDocument::fromJson(bytes, &error);
  if (error.error != QJsonParseError::NoError || !document.isObject())
    return std::nullopt;
  const auto root = document.object();
  if (!keys(root, {"schemaVersion", "kind", "source", "authorization",
                   "physicalQualification", "backend", "controlPlane",
                   "dataPlane", "advertisement", "receiverAvailable",
                   "liveAirdropInteroperability", "blockReason", "radio",
                   "upstreamVersion"}) ||
      root.value("schemaVersion") != QJsonValue(1) ||
      root.value("kind") != QJsonValue("airdrop_backend_status") ||
      root.value("authorization") != QJsonValue("never_evaluated") ||
      root.value("physicalQualification") != QJsonValue("not_qualified") ||
      !oneOf(root.value("source"),
             {"local_sysfs", "owly_doctor", "owly_process", "unknown"}) ||
      !oneOf(root.value("backend"), states()) ||
      !oneOf(root.value("controlPlane"), states()) ||
      !oneOf(root.value("dataPlane"), states()) ||
      !oneOf(root.value("advertisement"), states()) ||
      !oneOf(root.value("liveAirdropInteroperability"), states()) ||
      !root.value("receiverAvailable").isBool() ||
      !root.value("radio").isObject())
    return std::nullopt;

  const auto reason = root.value("blockReason");
  static const QStringList reasons{
      "backend_not_installed", "backend_version_unsupported",
      "radio_not_prepared", "radio_state_unknown", "backend_probe_failed",
      "permission_unavailable", "destination_unavailable",
      "interactive_consent_required", "concurrent_receive", "session_expired",
      "cancelled", "output_limit_exceeded", "backend_exited",
      "backend_failed"};
  if (!reason.isNull() && !oneOf(reason, reasons))
    return std::nullopt;

  const auto version = root.value("upstreamVersion");
  static const QRegularExpression versionPattern("^[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}$");
  if (!version.isNull() &&
      (!version.isString() || !versionPattern.match(version.toString()).hasMatch()))
    return std::nullopt;

  const auto radio = root.value("radio").toObject();
  if (!keys(radio, {"state", "kernelRelease", "driver", "pciId",
                    "regulatoryDomain", "wifiChannel", "associated",
                    "monitorMode", "wirelessInterfaceCount",
                    "conflictingWirelessInterfaceCount", "awdlInterfaceState",
                    "awdlLinkLocalAddressPresent", "effectiveUidIsRoot",
                    "netAdminCapability", "netRawCapability"}) ||
      !oneOf(radio.value("state"), states()) ||
      !oneOf(radio.value("monitorMode"), states()) ||
      !oneOf(radio.value("awdlInterfaceState"), states()) ||
      !radio.value("awdlLinkLocalAddressPresent").isBool() ||
      !radio.value("effectiveUidIsRoot").isBool() ||
      !radio.value("netAdminCapability").isBool() ||
      !radio.value("netRawCapability").isBool() ||
      !integer(radio.value("wirelessInterfaceCount"), 0, 255) ||
      !integer(radio.value("conflictingWirelessInterfaceCount"), 0, 255))
    return std::nullopt;

  AirdropStatusReport result;
  result.source = root.value("source").toString();
  result.backend = root.value("backend").toString();
  result.controlPlane = root.value("controlPlane").toString();
  result.dataPlane = root.value("dataPlane").toString();
  result.advertisement = root.value("advertisement").toString();
  result.blockReason = reason.isNull() ? "none" : reason.toString();
  result.radioState = radio.value("state").toString();
  result.monitorMode = radio.value("monitorMode").toString();
  result.awdlState = radio.value("awdlInterfaceState").toString();
  result.receiverAvailable = root.value("receiverAvailable").toBool();
  result.wirelessInterfaceCount =
      radio.value("wirelessInterfaceCount").toInt();
  result.conflictingInterfaceCount =
      radio.value("conflictingWirelessInterfaceCount").toInt();

  static const QRegularExpression driverPattern("^[A-Za-z0-9_.+-]{1,32}$");
  static const QRegularExpression pciPattern("^[0-9a-f]{4}:[0-9a-f]{4}$");
  static const QRegularExpression kernelPattern("^[A-Za-z0-9.+_-]{1,64}$");
  static const QRegularExpression countryPattern("^[A-Z]{2}$");
  if (!optionalText(radio.value("driver"), driverPattern, result.driver) ||
      !optionalText(radio.value("pciId"), pciPattern, result.pciId) ||
      !optionalText(radio.value("kernelRelease"), kernelPattern,
                    result.kernelRelease) ||
      !optionalText(radio.value("regulatoryDomain"), countryPattern,
                    result.regulatoryDomain))
    return std::nullopt;

  const auto channel = radio.value("wifiChannel");
  if (channel.isNull()) {
    result.channel = "unknown";
  } else if (integer(channel, 1, 233)) {
    result.channel = QString::number(channel.toInt());
  } else {
    return std::nullopt;
  }
  const auto associated = radio.value("associated");
  if (associated.isNull())
    result.associated = "unknown";
  else if (associated.isBool())
    result.associated = associated.toBool() ? "yes" : "no";
  else
    return std::nullopt;

  if (result.receiverAvailable !=
      (result.backend == "ready" && result.awdlState == "ready" &&
       result.dataPlane == "available"))
    return std::nullopt;
  return result;
}

QVariantMap AirdropStatusReport::asMap() const {
  return {{"source", source},
          {"backend", backend},
          {"controlPlane", controlPlane},
          {"dataPlane", dataPlane},
          {"advertisement", advertisement},
          {"blockReason", blockReason},
          {"radioState", radioState},
          {"driver", driver},
          {"pciId", pciId},
          {"kernelRelease", kernelRelease},
          {"regulatoryDomain", regulatoryDomain},
          {"monitorMode", monitorMode},
          {"awdlState", awdlState},
          {"channel", channel},
          {"associated", associated},
          {"wirelessInterfaceCount", wirelessInterfaceCount},
          {"conflictingInterfaceCount", conflictingInterfaceCount},
          {"receiverAvailable", receiverAvailable}};
}
