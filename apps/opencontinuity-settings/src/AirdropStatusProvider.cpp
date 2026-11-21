#include "AirdropStatusProvider.h"
#include <QFileInfo>

namespace {
constexpr qsizetype kMaxStdout = 64 * 1024;
constexpr qsizetype kMaxStderr = 4 * 1024;
}

AirdropStatusProvider::AirdropStatusProvider(Options options, QObject *parent)
    : QObject(parent), m_options(std::move(options)) {
  m_deadline.setSingleShot(true);
  connect(&m_deadline, &QTimer::timeout, this, [this] {
    if (m_process)
      fail("timed_out", "The read-only backend check exceeded its deadline.");
  });
}

AirdropStatusProvider::~AirdropStatusProvider() { retire(); }

bool AirdropStatusProvider::current(QProcess *process,
                                    quint64 generation) const {
  return process == m_process && generation == m_generation;
}

void AirdropStatusProvider::retire() {
  ++m_generation;
  m_deadline.stop();
  if (!m_process)
    return;
  auto *process = m_process.data();
  m_process = nullptr;
  process->disconnect(this);
  if (process->state() == QProcess::NotRunning)
    process->deleteLater();
  else {
    connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
            process, &QObject::deleteLater);
    process->kill();
  }
}

void AirdropStatusProvider::fail(const QString &state, const QString &message) {
  retire();
  m_state = state;
  m_error = message;
  emit changed();
}

void AirdropStatusProvider::cancel() {
  if (m_process)
    fail("cancelled", "Status refresh cancelled; no new report was accepted.");
}

void AirdropStatusProvider::read(QProcess *process, quint64 generation,
                                 bool stderrChannel) {
  if (!current(process, generation))
    return;
  process->setReadChannel(stderrChannel ? QProcess::StandardError
                                        : QProcess::StandardOutput);
  auto &buffer = stderrChannel ? m_stderr : m_stdout;
  const qsizetype maximum = stderrChannel ? kMaxStderr : kMaxStdout;
  buffer += process->read(maximum - buffer.size() + 1);
  if (buffer.size() > maximum)
    fail("invalid_report", "The backend status output exceeded its size limit.");
}

void AirdropStatusProvider::refresh() {
  if (loading())
    return;
  const QFileInfo file(m_options.executable);
  if (!file.isAbsolute() || !file.isFile() || !file.isExecutable()) {
    fail("cli_unavailable",
         "OpenContinuity CLI is unavailable at the configured absolute path.");
    return;
  }
  m_stdout.clear();
  m_stderr.clear();
  m_error.clear();
  m_state = "loading";
  auto *process = new QProcess(this);
  m_process = process;
  const auto generation = ++m_generation;
  process->setProgram(file.absoluteFilePath());
  process->setArguments({"airdrop", "doctor", "--json"});
  process->setProcessChannelMode(QProcess::SeparateChannels);
  connect(process, &QProcess::readyReadStandardOutput, this,
          [this, process, generation] { read(process, generation, false); });
  connect(process, &QProcess::readyReadStandardError, this,
          [this, process, generation] { read(process, generation, true); });
  connect(process, &QProcess::errorOccurred, this,
          [this, process, generation](QProcess::ProcessError error) {
            if (current(process, generation))
              fail(error == QProcess::FailedToStart ? "cli_unavailable"
                                                    : "process_error",
                   "The backend status command did not complete.");
          });
  connect(process, qOverload<int, QProcess::ExitStatus>(&QProcess::finished),
          this, [this, process, generation](int code,
                                            QProcess::ExitStatus exit) {
            if (!current(process, generation))
              return;
            read(process, generation, false);
            read(process, generation, true);
            if (!current(process, generation))
              return;
            if (exit != QProcess::NormalExit || code != 0 || !m_stderr.isEmpty()) {
              fail("process_error",
                   "The CLI did not return a clean read-only status result.");
              return;
            }
            const auto report = AirdropStatusReport::parse(m_stdout);
            if (!report) {
              fail("invalid_report",
                   "The CLI returned an invalid schema-v1 backend report.");
              return;
            }
            retire();
            m_report = *report;
            m_state = "loaded";
            m_error.clear();
            emit changed();
          });
  m_deadline.start(m_options.timeoutMs);
  process->start(QIODevice::ReadOnly);
  emit changed();
}
