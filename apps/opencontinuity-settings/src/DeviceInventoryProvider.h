#pragma once
#include "DeviceInventoryModel.h"
#include <QDateTime>
#include <QPointer>
#include <QProcess>
#include <QTimer>

class DeviceInventoryProvider final : public QObject {
  Q_OBJECT
  Q_PROPERTY(DeviceInventoryModel *devices READ devices CONSTANT)
  Q_PROPERTY(bool loading READ loading NOTIFY changed)
  Q_PROPERTY(QString state READ state NOTIFY changed)
  Q_PROPERTY(QString status READ status NOTIFY changed)
  Q_PROPERTY(QString lastError READ lastError NOTIFY changed)
  Q_PROPERTY(QString lastUpdated READ lastUpdated NOTIFY changed)
  Q_PROPERTY(int adapterCount READ adapterCount NOTIFY changed)
  Q_PROPERTY(QString executable READ executable CONSTANT)
public:
  struct Options { QString executable; int timeoutMs = 3000; };
  explicit DeviceInventoryProvider(Options options, QObject *parent = nullptr);
  ~DeviceInventoryProvider() override;
  DeviceInventoryModel *devices() { return &m_devices; }
  bool loading() const { return m_process; }
  QString state() const { return m_state; }
  QString status() const { return m_status; }
  QString lastError() const { return m_error; }
  QString lastUpdated() const;
  int adapterCount() const { return m_adapters; }
  QString executable() const { return m_options.executable; }
  Q_INVOKABLE void refresh();
  Q_INVOKABLE void cancel();
signals:
  void changed();
private:
  bool current(QProcess *process, quint64 generation) const;
  void read(QProcess *process, quint64 generation, bool stderrChannel);
  void fail(const QString &state, const QString &message);
  void retire();
  Options m_options;
  DeviceInventoryModel m_devices;
  QPointer<QProcess> m_process;
  QTimer m_deadline, m_age;
  quint64 m_generation = 0;
  QByteArray m_stdout, m_stderr;
  QString m_state = "idle", m_status = "not_refreshed", m_error;
  QDateTime m_updated;
  int m_adapters = 0;
};
