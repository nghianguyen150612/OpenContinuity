#pragma once
#include "DeviceDiscoveryModel.h"
#include <QSet>

struct DeviceDiscoveryReport {
  QString status, cleanup, sessionId, sessionStatus;
  bool started = false, completed = false, sessionSeen = false, terminalSeen = false;
  int requestedSeconds = 0, actualMs = 0, deviceEventCount = 0;

  void reset();
  bool consumeLine(const QByteArray &line, DeviceDiscoveryModel &model,
                  QString *error = nullptr);
  bool finish(int exitCode, bool normalExit, const DeviceDiscoveryModel &model,
              QString *error = nullptr) const;

private:
  int m_nextSequence = 1;
  int m_adapterIndex = 0;
  QSet<QString> m_ids;
};
