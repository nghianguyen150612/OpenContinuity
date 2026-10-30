#include "DoctorProvider.h"
#include <QFileInfo>
DoctorProvider::DoctorProvider(Options options, QObject *parent)
    : QObject(parent), m_options(std::move(options)), m_capabilities(this),
      m_trust(this) {
  m_deadline.setSingleShot(true);
  m_age.setSingleShot(true);
  connect(&m_deadline, &QTimer::timeout, this, [this] {
    if (m_process)
      fail("timed_out", "CLI exceeded the report deadline. Try Refresh.");
  });
  connect(&m_age, &QTimer::timeout, this, [this] {
    if (m_updated.isValid() && !loading()) {
      m_state = "stale";
      m_error = "Report is over five minutes old. Refresh to load current "
                "build information.";
      emit changed();
    }
  });
}
DoctorProvider::~DoctorProvider() { retire(); }
bool DoctorProvider::current(QProcess *p, quint64 g) const {
  return p == m_process && g == m_generation;
}
void DoctorProvider::retire() {
  ++m_generation;
  m_deadline.stop();
  if (!m_process)
    return;
  auto *p = m_process.data();
  m_process = nullptr;
  p->disconnect(this);
  if (p->state() == QProcess::NotRunning)
    p->deleteLater();
  else {
    connect(p, qOverload<int, QProcess::ExitStatus>(&QProcess::finished), p,
            &QObject::deleteLater);
    p->kill();
  }
}
void DoctorProvider::fail(const QString &state, const QString &message) {
  retire();
  m_state = m_updated.isValid() ? "stale" : state;
  m_failure = state;
  m_error = message;
  emit changed();
}
void DoctorProvider::cancel() {
  if (m_process)
    fail("cancelled",
         "Refresh cancelled. Previous information, if any, is stale.");
}
void DoctorProvider::read(QProcess *p, quint64 g, bool stderrChannel) {
  if (!current(p, g))
    return;
  p->setReadChannel(stderrChannel ? QProcess::StandardError
                                  : QProcess::StandardOutput);
  auto &buffer = stderrChannel ? m_stderr : m_stdout;
  const qsizetype maximum = stderrChannel ? 4096 : DoctorReport::MaxOutput;
  buffer += p->read(maximum - buffer.size() + 1);
  if (buffer.size() > maximum)
    fail("invalid_report", "CLI output exceeded the safe size limit.");
}
void DoctorProvider::refresh() {
  if (loading())
    return;
  m_age.stop();
  const QFileInfo file(m_options.executable);
  if (!file.isAbsolute() || !file.isFile() || !file.isExecutable()) {
    fail("cli_unavailable",
         "CLI unavailable. Install opencontinuity beside this app or launch "
         "with --cli /absolute/path/opencontinuity.");
    return;
  }
  m_stdout.clear();
  m_stderr.clear();
  m_error.clear();
  m_failure.clear();
  m_state = "loading";
  auto *p = new QProcess(this);
  m_process = p;
  const auto generation = ++m_generation;
  p->setProgram(file.absoluteFilePath());
  p->setArguments({"doctor", "--json"});
  p->setProcessChannelMode(QProcess::SeparateChannels);
  connect(p, &QProcess::readyReadStandardOutput, this,
          [this, p, generation] { read(p, generation, false); });
  connect(p, &QProcess::readyReadStandardError, this,
          [this, p, generation] { read(p, generation, true); });
  connect(p, &QProcess::errorOccurred, this,
          [this, p, generation](QProcess::ProcessError e) {
            if (current(p, generation))
              fail(e == QProcess::FailedToStart ? "cli_unavailable"
                                                : "process_error",
                   "CLI could not complete the report. Check the executable "
                   "and retry.");
          });
  connect(p, qOverload<int, QProcess::ExitStatus>(&QProcess::finished), this,
          [this, p, generation](int code, QProcess::ExitStatus exit) {
            if (!current(p, generation))
              return;
            read(p, generation, false);
            read(p, generation, true);
            if (!current(p, generation))
              return;
            if (exit != QProcess::NormalExit || code != 0) {
              fail("process_error",
                   "CLI exited unsuccessfully. No new report was accepted.");
              return;
            }
            const auto report = DoctorReport::parse(m_stdout);
            if (!report || !m_stderr.isEmpty()) {
              fail("invalid_report", "CLI returned an invalid schema-v1 report "
                                     "or unexpected diagnostic output.");
              return;
            }
            retire();
            m_capabilities.replace(report->capabilities);
            m_trust.replace(report->trust);
            m_version = report->version;
            m_updated = QDateTime::currentDateTimeUtc();
            m_state = "loaded";
            m_failure.clear();
            m_error.clear();
            m_age.start(m_options.staleMs);
            emit changed();
          });
  m_deadline.start(m_options.timeoutMs);
  p->start(QIODevice::ReadOnly);
  emit changed();
}
