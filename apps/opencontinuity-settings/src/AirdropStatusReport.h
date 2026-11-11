#pragma once

#include <QVariantMap>
#include <optional>

struct AirdropStatusReport {
  QString source = "unknown";
  QString backend = "unknown";
  QString controlPlane = "unknown";
  QString dataPlane = "unknown";
  QString advertisement = "unknown";
  QString blockReason = "unknown";
  QString radioState = "unknown";
  QString driver = "unknown";
  QString pciId = "unknown";
  QString kernelRelease = "unknown";
  QString regulatoryDomain = "unknown";
  QString monitorMode = "unknown";
  QString awdlState = "unknown";
  QString channel = "unknown";
  QString associated = "unknown";
  int wirelessInterfaceCount = 0;
  int conflictingInterfaceCount = 0;
  bool receiverAvailable = false;

  static std::optional<AirdropStatusReport> parse(const QByteArray &bytes);
  QVariantMap asMap() const;
};
