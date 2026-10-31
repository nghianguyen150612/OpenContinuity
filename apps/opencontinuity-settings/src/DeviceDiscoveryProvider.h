#pragma once
#include "DeviceDiscoveryModel.h"
#include <QElapsedTimer>
#include <QPointer>
#include <QProcess>
#include <QTimer>
class DeviceDiscoveryProvider final : public QObject {
  Q_OBJECT
  Q_PROPERTY(DeviceDiscoveryModel *devices READ devices CONSTANT)
  Q_PROPERTY(bool scanning READ scanning NOTIFY changed)
  Q_PROPERTY(bool cancelling READ cancelling NOTIFY changed)
  Q_PROPERTY(QString state READ state NOTIFY changed)
  Q_PROPERTY(QString message READ message NOTIFY changed)
  Q_PROPERTY(QString cleanupStatus READ cleanupStatus NOTIFY changed)
  Q_PROPERTY(double elapsedSeconds READ elapsedSeconds NOTIFY changed)
  Q_PROPERTY(int requestedSeconds READ requestedSeconds CONSTANT)
public:
  struct Options { QString executable; int timeoutMs = 22000; int cancelGraceMs = 6500; };
  explicit DeviceDiscoveryProvider(Options options, QObject *parent = nullptr);
  ~DeviceDiscoveryProvider() override;
  DeviceDiscoveryModel *devices() { return &m_devices; }
  bool scanning() const { return !m_process.isNull(); }
  bool cancelling() const { return m_cancelling; }
  QString state() const { return m_state; }
  QString message() const { return m_message; }
  QString cleanupStatus() const { return m_cleanup; }
  double elapsedSeconds() const;
  int requestedSeconds() const { return 10; }
  Q_INVOKABLE void scan();
  Q_INVOKABLE void cancel();
  void shutdown();
signals:
  void changed();
private:
  bool current(QProcess *p, quint64 g) const;
  void read(QProcess *p, quint64 g, bool error);
  void reject(const QString &message);
  void retire();
  Options m_options;
  DeviceDiscoveryModel m_devices;
  QPointer<QProcess> m_process;
  QTimer m_deadline, m_fallback, m_progress, m_age;
  QElapsedTimer m_elapsed;
  quint64 m_generation = 0;
  QByteArray m_stdout, m_stderr;
  QString m_state = "idle", m_message = "No scan requested.", m_cleanup = "not_needed";
  QString m_failure;
  bool m_cancelling = false;
  double m_actualSeconds = 0;
};
