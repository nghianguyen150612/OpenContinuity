#pragma once
#include "DeviceDiscoveryModel.h"
#include <optional>
struct DeviceDiscoveryReport {
  QString status, cleanup;
  bool started = false, completed = false;
  int requestedSeconds = 0, actualMs = 0;
  QList<DiscoveryRow> devices;
  static std::optional<DeviceDiscoveryReport> parse(const QByteArray &bytes);
};
