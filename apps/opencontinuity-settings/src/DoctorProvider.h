#pragma once
#include "StatusModel.h"
#include <QDateTime>
#include <QPointer>
#include <QProcess>
#include <QTimer>

class DoctorProvider : public QObject {
  Q_OBJECT
  Q_PROPERTY(StatusModel *capabilities READ capabilities CONSTANT)
  Q_PROPERTY(StatusModel *trustStages READ trustStages CONSTANT)
  Q_PROPERTY(bool loading READ loading NOTIFY changed)
  Q_PROPERTY(QString reportState READ reportState NOTIFY changed)
  Q_PROPERTY(QString failureState READ failureState NOTIFY changed)
  Q_PROPERTY(QString lastError READ lastError NOTIFY changed)
  Q_PROPERTY(QString reportVersion READ reportVersion NOTIFY changed)
  Q_PROPERTY(int schemaVersion READ schemaVersion NOTIFY changed)
  Q_PROPERTY(QString lastUpdated READ lastUpdated NOTIFY changed)
  Q_PROPERTY(QString executable READ executable CONSTANT)
public:
  struct Options {
    QString executable;
    int timeoutMs = 3000;
    int staleMs = 300000;
  };
  explicit DoctorProvider(Options options, QObject *parent = nullptr);
  ~DoctorProvider() override;
  StatusModel *capabilities() { return &m_capabilities; }
  StatusModel *trustStages() { return &m_trust; }
  bool loading() const { return m_process; }
  QString reportState() const { return m_state; }
  QString failureState() const { return m_failure; }
  QString lastError() const { return m_error; }
  QString reportVersion() const { return m_version; }
  int schemaVersion() const { return m_updated.isValid() ? 1 : 0; }
  QString lastUpdated() const {
    return m_updated.isValid() ? m_updated.toLocalTime().toString(Qt::ISODate)
                               : QString();
  }
  QString executable() const { return m_options.executable; }
  Q_INVOKABLE QVariantMap capability(const QString &id) const {
    return m_capabilities.capability(id);
  }
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
  StatusModel m_capabilities, m_trust;
  QPointer<QProcess> m_process;
  QTimer m_deadline, m_age;
  quint64 m_generation = 0;
  QByteArray m_stdout, m_stderr;
  QString m_state = "unavailable", m_failure, m_error, m_version;
  QDateTime m_updated;
};
