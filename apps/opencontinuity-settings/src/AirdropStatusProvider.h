#pragma once

#include "AirdropStatusReport.h"
#include <QPointer>
#include <QProcess>
#include <QTimer>

class AirdropStatusProvider final : public QObject {
  Q_OBJECT
  Q_PROPERTY(QString state READ state NOTIFY changed)
  Q_PROPERTY(bool loading READ loading NOTIFY changed)
  Q_PROPERTY(QVariantMap report READ report NOTIFY changed)
  Q_PROPERTY(QString lastError READ lastError NOTIFY changed)
public:
  struct Options {
    QString executable;
    int timeoutMs = 10000;
  };
  explicit AirdropStatusProvider(Options options, QObject *parent = nullptr);
  ~AirdropStatusProvider() override;
  QString state() const { return m_state; }
  bool loading() const { return m_process; }
  QVariantMap report() const { return m_report.asMap(); }
  QString lastError() const { return m_error; }
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
  AirdropStatusReport m_report;
  QPointer<QProcess> m_process;
  QTimer m_deadline;
  quint64 m_generation = 0;
  QByteArray m_stdout, m_stderr;
  QString m_state = "not_refreshed", m_error;
};
