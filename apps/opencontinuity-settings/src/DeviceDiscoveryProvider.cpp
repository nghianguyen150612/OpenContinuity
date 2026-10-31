#include "DeviceDiscoveryProvider.h"
#include <QFileInfo>

namespace {
constexpr qsizetype kMaxOutputBytes = 512 * 1024;
constexpr qsizetype kMaxRecordBytes = 2 * 1024;
constexpr qsizetype kReadBufferBytes = 8 * 1024;

QString outcome(const QString &status, bool empty) {
  if (status == "completed") return empty ? "Scan completed. No observations. This does not prove that no devices are in range." : "Scan completed. These are unverified BlueZ events, not authenticated device identities.";
  if (status == "cancelled") return "Scan cancelled. Results are partial and stale.";
  if (status == "no_adapter") return "No Bluetooth adapter is available.";
  if (status == "adapter_not_powered") return "Adapter not powered. Enable Bluetooth yourself before scanning.";
  if (status == "access_denied") return "Permission denied for Bluetooth discovery.";
  if (status == "unsupported_le") return "LE-only discovery is unsupported by this adapter or BlueZ.";
  if (status == "adapter_busy") return "Adapter busy with an OpenContinuity radio session.";
  if (status == "bluez_unavailable") return "BlueZ is unavailable or its owner changed.";
  if (status == "system_bus_unavailable") return "The system D-Bus is unavailable.";
  if (status == "unsupported_platform") return "Bluetooth discovery is unavailable on this platform.";
  if (status == "timeout") return "Scan timed out before it could complete.";
  if (status == "data_limit") return "Scan stopped at the bounded event, device, or output limit. Results are partial.";
  if (status == "output_blocked") return "The scan stopped because its event output could not be delivered.";
  if (status == "output_closed") return "The scan stopped because the event output consumer closed the stream.";
  if (status == "cleanup_incomplete") return "Cleanup incomplete. The scan could not confirm that its session was released.";
  return "Scan failed. BlueZ returned malformed data.";
}
}

DeviceDiscoveryProvider::DeviceDiscoveryProvider(Options options, QObject *parent)
    : QObject(parent), m_options(std::move(options)), m_devices(this) {
  m_deadline.setSingleShot(true); m_fallback.setSingleShot(true); m_age.setSingleShot(true);
  m_progress.setInterval(100);
  connect(&m_progress, &QTimer::timeout, this, &DeviceDiscoveryProvider::changed);
  connect(&m_age, &QTimer::timeout, this, [this] { m_devices.markStale(); emit changed(); });
  connect(&m_deadline, &QTimer::timeout, this, [this] { if (scanning()) { m_failure = "Scan exceeded its deadline."; cancel(); } });
  connect(&m_fallback, &QTimer::timeout, this, [this] { if (scanning()) { m_failure = "CLI did not finish graceful cleanup before the deadline."; m_process->kill(); } });
}

DeviceDiscoveryProvider::~DeviceDiscoveryProvider() { shutdown(); }
double DeviceDiscoveryProvider::elapsedSeconds() const { return scanning() && m_elapsed.isValid() ? m_elapsed.elapsed() / 1000.0 : m_actualSeconds; }
bool DeviceDiscoveryProvider::current(QProcess *p, quint64 g) const { return p == m_process && g == m_generation; }

void DeviceDiscoveryProvider::retire() {
  ++m_generation; m_deadline.stop(); m_fallback.stop(); m_progress.stop();
  if (!m_process) return;
  auto *p = m_process.data(); m_process = nullptr; p->disconnect(this); p->deleteLater(); m_cancelling = false;
}

void DeviceDiscoveryProvider::cancel() {
  if (!scanning() || m_cancelling) return;
  m_cancelling = true; m_state = "cancelling"; m_message = "Cancelling scan; waiting for the CLI cleanup report…";
  m_devices.markStale();
  if (m_process->state() == QProcess::Running) m_process->terminate();
  m_fallback.start(m_options.cancelGraceMs); emit changed();
}

void DeviceDiscoveryProvider::shutdown() {
  if (!m_process) return;
  cancel();
  const QPointer<QProcess> process = m_process;
  const auto generation = m_generation;
  if (process && process->state() == QProcess::Starting) process->waitForStarted(1000);
  if (process && process->state() == QProcess::Running &&
      !process->waitForFinished(m_options.cancelGraceMs)) {
    m_failure = "The CLI was killed before it confirmed cleanup.";
    process->kill();
    process->waitForFinished(1000);
  }
  if (process && current(process, generation)) {
    read(process, generation, false); read(process, generation, true);
    if (process->state() == QProcess::NotRunning) {
      complete(process, generation, process->exitCode(), process->exitStatus());
    } else {
      reject("Cleanup could not be confirmed during window shutdown."); retire();
    }
  }
}

void DeviceDiscoveryProvider::reject(const QString &message) {
  m_state = "failed"; m_message = message; m_cleanup = "incomplete"; m_devices.markStale(); emit changed();
}

void DeviceDiscoveryProvider::read(QProcess *process, quint64 generation, bool error) {
  if (!current(process, generation)) return;
  process->setReadChannel(error ? QProcess::StandardError : QProcess::StandardOutput);
  if (error) {
    const auto bytes = process->read(4097);
    if (!bytes.isEmpty()) {
      const auto remaining = qMax<qsizetype>(0, 4096 - m_stderr.size());
      m_stderr += bytes.left(remaining);
      if (m_failure.isEmpty()) m_failure = "CLI wrote unexpected diagnostic output.";
      if (!m_cancelling) cancel();
      if (bytes.size() > remaining) process->closeReadChannel(QProcess::StandardError);
    }
    return;
  }

  while (true) {
    const auto bytes = process->read(kReadBufferBytes);
    if (bytes.isEmpty()) break;
    m_totalOutputBytes += bytes.size();
    if (m_totalOutputBytes > kMaxOutputBytes) {
      if (m_failure.isEmpty()) m_failure = "CLI JSONL output exceeded its total size limit.";
      process->closeReadChannel(QProcess::StandardOutput);
      if (!m_cancelling) cancel();
      return;
    }
    if (!m_failure.isEmpty()) continue;
    m_lineBuffer += bytes;
    while (true) {
      const auto newline = m_lineBuffer.indexOf('\n');
      if (newline < 0) break;
      const auto line = m_lineBuffer.left(newline);
      m_lineBuffer.remove(0, newline + 1);
      QString parseError;
      if (!m_report.consumeLine(line, m_devices, &parseError)) {
        m_failure = parseError;
        if (!m_cancelling) cancel();
        break;
      }
      if (m_report.sessionSeen && m_state == "scanning") {
        m_message = "Live BlueZ observations are unverified. No proximity or device identity is established.";
        emit changed();
      }
    }
    if (m_lineBuffer.size() > kMaxRecordBytes) {
      m_failure = "CLI JSONL record exceeded its line size limit.";
      process->closeReadChannel(QProcess::StandardOutput);
      if (!m_cancelling) cancel();
      return;
    }
  }
}

void DeviceDiscoveryProvider::complete(QProcess *process, quint64 generation,
                                       int code, QProcess::ExitStatus exit) {
  if (!current(process, generation)) return;
  read(process, generation, false); read(process, generation, true);
  QString validationError;
  const bool valid = m_failure.isEmpty() && m_lineBuffer.isEmpty() &&
                     m_stderr.isEmpty() &&
                     m_report.finish(code, exit == QProcess::NormalExit,
                                     m_devices, &validationError);
  const auto failure = m_failure;
  retire();
  if (!valid) {
    reject(!failure.isEmpty() ? failure + " Cleanup is uncertain." : validationError + " Cleanup is uncertain.");
    return;
  }
  m_state = m_report.status; m_cleanup = m_report.cleanup;
  m_actualSeconds = m_report.actualMs / 1000.0; m_durationKnown = true;
  m_message = outcome(m_report.status, m_devices.count() == 0);
  if (!m_report.completed || m_report.cleanup != "complete") m_devices.markStale();
  m_age.start(60000); emit changed();
}

void DeviceDiscoveryProvider::scan() {
  if (scanning()) return;
  m_age.stop(); m_devices.clear(); m_report.reset(); m_durationKnown = false;
  const QFileInfo file(m_options.executable);
  if (!file.isAbsolute() || !file.isFile() || !file.isExecutable()) {
    m_state = "cli_unavailable"; m_message = "CLI unavailable. Install opencontinuity beside this app or use --cli /absolute/path/opencontinuity."; m_cleanup = "not_needed"; emit changed(); return;
  }
  m_lineBuffer.clear(); m_stderr.clear(); m_totalOutputBytes = 0; m_failure.clear(); m_cancelling = false;
  m_state = "scanning"; m_message = "Starting an explicit 10-second Bluetooth LE scan…"; m_cleanup = "pending";
  m_elapsed.start(); ++m_generation;
  auto *process = new QProcess(this); m_process = process; const auto generation = m_generation;
  process->setProgram(file.absoluteFilePath());
  process->setArguments({"devices", "discover", "--seconds", "10", "--events-jsonl"});
  process->setProcessChannelMode(QProcess::SeparateChannels);
  process->setReadBufferSize(kReadBufferBytes);
  connect(process, &QProcess::started, this, [this,process,generation] { if (current(process,generation) && m_cancelling) process->terminate(); });
  connect(process, &QProcess::readyReadStandardOutput, this, [this,process,generation] { read(process,generation,false); });
  connect(process, &QProcess::readyReadStandardError, this, [this,process,generation] { read(process,generation,true); });
  connect(process, &QProcess::errorOccurred, this, [this,process,generation](QProcess::ProcessError error) {
    if (!current(process,generation)) return;
    if (error == QProcess::FailedToStart) { retire(); m_state = "cli_unavailable"; m_message = "CLI could not start."; m_cleanup = "not_needed"; emit changed(); }
  });
  connect(process, qOverload<int,QProcess::ExitStatus>(&QProcess::finished), this,
          [this,process,generation](int code, QProcess::ExitStatus exit) {
            complete(process,generation,code,exit);
          });
  m_deadline.start(m_options.timeoutMs); m_progress.start(); process->start(QIODevice::ReadOnly); emit changed();
}
