#include "DeviceDiscoveryProvider.h"
#include "DeviceDiscoveryReport.h"
#include <QFileInfo>
namespace {
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
  if (status == "data_limit") return "Scan stopped at the bounded event or device limit. Results are partial.";
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
  m_cancelling = true; m_state = "cancelling"; m_message = "Cancelling scan; waiting for bounded CLI cleanup…";
  m_devices.markStale();
  // SIGTERM on Unix; Rust handles it and emits the final cleanup report.
  if (m_process->state() == QProcess::Running) m_process->terminate();
  m_fallback.start(m_options.cancelGraceMs); emit changed();
}
void DeviceDiscoveryProvider::shutdown() {
  if (!m_process) return;
  cancel();
  // The event loop may be exiting. Preserve graceful cleanup on application close.
  const QPointer<QProcess> p = m_process;
  if (p && p->state() == QProcess::Starting) p->waitForStarted(1000);
  if (p && p->state() == QProcess::Running) {
    p->terminate();
    if (!p->waitForFinished(m_options.cancelGraceMs)) { p->kill(); p->waitForFinished(1000); }
  }
  if (m_process) { reject("Cleanup could not be confirmed during shutdown."); retire(); }
}
void DeviceDiscoveryProvider::reject(const QString &message) {
  m_state = "failed"; m_message = message; m_cleanup = "incomplete"; m_devices.markStale(); emit changed();
}
void DeviceDiscoveryProvider::read(QProcess *p, quint64 g, bool error) {
  if (!current(p, g)) return;
  p->setReadChannel(error ? QProcess::StandardError : QProcess::StandardOutput);
  auto &bytes = error ? m_stderr : m_stdout; const qsizetype maximum = error ? 4096 : 65536;
  bytes += p->read(qMax<qsizetype>(0, maximum - bytes.size() + 1));
  if (bytes.size() > maximum && m_failure.isEmpty()) { m_failure = "CLI output exceeded its bounded size limit."; cancel(); }
  if (bytes.size() > maximum) p->closeReadChannel(error ? QProcess::StandardError : QProcess::StandardOutput);
}
void DeviceDiscoveryProvider::scan() {
  if (scanning()) return;
  m_age.stop(); m_devices.markStale(); m_durationKnown = false;
  const QFileInfo file(m_options.executable);
  if (!file.isAbsolute() || !file.isFile() || !file.isExecutable()) {
    m_state = "cli_unavailable"; m_message = "CLI unavailable. Install opencontinuity beside this app or use --cli /absolute/path/opencontinuity."; emit changed(); return;
  }
  m_stdout.clear(); m_stderr.clear(); m_failure.clear(); m_cancelling = false;
  m_state = "scanning"; m_message = "Scan in progress. Elapsed time includes adapter setup; the radio interval is 10 seconds."; m_cleanup = "pending";
  m_elapsed.start(); ++m_generation;
  auto *p = new QProcess(this); m_process = p; const auto generation = m_generation;
  p->setProgram(file.absoluteFilePath()); p->setArguments({"devices", "discover", "--seconds", "10", "--json"});
  p->setProcessChannelMode(QProcess::SeparateChannels);
  connect(p, &QProcess::started, this, [this,p,generation] { if (current(p,generation) && m_cancelling) p->terminate(); });
  connect(p, &QProcess::readyReadStandardOutput, this, [this,p,generation] { read(p,generation,false); });
  connect(p, &QProcess::readyReadStandardError, this, [this,p,generation] { read(p,generation,true); });
  connect(p, &QProcess::errorOccurred, this, [this,p,generation](QProcess::ProcessError error) {
    if (!current(p,generation)) return;
    if (error == QProcess::FailedToStart) { retire(); m_state = "cli_unavailable"; m_message = "CLI could not start."; m_cleanup = "not_needed"; emit changed(); }
  });
  connect(p, qOverload<int,QProcess::ExitStatus>(&QProcess::finished), this, [this,p,generation](int code, QProcess::ExitStatus exit) {
    if (!current(p,generation)) return;
    read(p,generation,false); read(p,generation,true);
    const auto parsed = DeviceDiscoveryReport::parse(m_stdout);
    const bool valid = parsed && m_stderr.isEmpty() && exit == QProcess::NormalExit && parsed->requestedSeconds == 10 &&
                       (parsed->status == "completed" ? code == 0 : code == 1);
    const auto failure = m_failure;
    retire();
    if (!valid || !failure.isEmpty()) {
      reject(failure.isEmpty() ? "CLI returned an invalid scan report. Cleanup is uncertain." : failure + " Cleanup is uncertain."); return;
    }
    m_devices.replace(parsed->devices); m_state = parsed->status; m_cleanup = parsed->cleanup; m_actualSeconds = parsed->actualMs / 1000.0; m_durationKnown = true;
    m_message = outcome(parsed->status, parsed->devices.isEmpty());
    if (!parsed->completed) m_devices.markStale();
    m_age.start(60000); emit changed();
  });
  m_deadline.start(m_options.timeoutMs); m_progress.start(); p->start(QIODevice::ReadOnly); emit changed();
}
