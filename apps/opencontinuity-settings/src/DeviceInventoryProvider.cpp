#include "DeviceInventoryProvider.h"
#include "JsonValidation.h"
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSet>
#include <optional>

namespace {
constexpr qsizetype MaxOutput = 64 * 1024;
constexpr qsizetype MaxStderr = 4096;
using namespace JsonValidation;
bool nullableBool(const QJsonValue &value, QVariant *out) {
  if (value.isNull()) { *out = {}; return true; }
  if (!value.isBool()) return false;
  *out = value.toBool(); return true;
}
struct Parsed { QString status; int adapters = 0; QList<DeviceInventoryRow> devices; };
std::optional<Parsed> parse(const QByteArray &bytes) {
  if (bytes.isEmpty() || bytes.size() > MaxOutput || !uniqueMembers(bytes)) return {};
  QJsonParseError error;
  const auto document = QJsonDocument::fromJson(bytes, &error);
  if (error.error != QJsonParseError::NoError || !document.isObject()) return {};
  const auto root = document.object();
  if (!keys(root, {"schema_version", "source", "scan_performed", "status", "adapters", "devices", "authorization"}) ||
      !root.value("schema_version").isDouble() || root.value("schema_version").toDouble() != 1.0 ||
      root.value("source") != QJsonValue("bluez_known_device_snapshot") ||
      !root.value("scan_performed").isBool() || root.value("scan_performed").toBool() ||
      root.value("authorization") != QJsonValue("never_evaluated") ||
      !root.value("adapters").isArray() || !root.value("devices").isArray()) return {};
  const QStringList statuses{"ok", "no_adapter", "bluez_unavailable", "system_bus_unavailable", "access_denied", "timeout", "malformed_data", "unsupported_platform"};
  const auto status = root.value("status").toString();
  const auto adapters = root.value("adapters").toArray();
  const auto devices = root.value("devices").toArray();
  if (!statuses.contains(status) || adapters.size() > 16 || devices.size() > 128) return {};
  if ((status == "ok" && adapters.isEmpty()) || (status == "no_adapter" && !adapters.isEmpty()) ||
      ((status != "ok" && status != "no_adapter") && (!adapters.isEmpty() || !devices.isEmpty()))) return {};
  Parsed parsed{status, static_cast<int>(adapters.size()), {}};
  QSet<QString> adapterIds;
  for (qsizetype i = 0; i < adapters.size(); ++i) {
    const auto value = adapters[i]; if (!value.isObject()) return {};
    const auto obj = value.toObject(); const auto id = QString("adapter-%1").arg(i + 1, 3, 10, QLatin1Char('0'));
    if (!keys(obj, {"id", "powered"}) || obj.value("id") != QJsonValue(id) || adapterIds.contains(id)) return {};
    QVariant powered; if (!nullableBool(obj.value("powered"), &powered)) return {};
    adapterIds.insert(id);
  }
  QSet<QString> deviceIds;
  for (qsizetype i = 0; i < devices.size(); ++i) {
    const auto value = devices[i]; if (!value.isObject()) return {};
    const auto obj = value.toObject(); const auto id = QString("device-%1").arg(i + 1, 3, 10, QLatin1Char('0'));
    if (!keys(obj, {"id", "adapter_id", "display_name", "connected", "paired", "trusted", "services_resolved"}) ||
        obj.value("id") != QJsonValue(id) || deviceIds.contains(id) || !safeText(obj.value("adapter_id"), 16) ||
        !adapterIds.contains(obj.value("adapter_id").toString()) || !safeText(obj.value("display_name"), 160)) return {};
    DeviceInventoryRow row; row.id = id; row.adapterId = obj.value("adapter_id").toString(); row.displayName = obj.value("display_name").toString();
    if (!nullableBool(obj.value("connected"), &row.connected) || !nullableBool(obj.value("paired"), &row.paired) ||
        !nullableBool(obj.value("trusted"), &row.trusted)) return {};
    QVariant unused; if (!nullableBool(obj.value("services_resolved"), &unused)) return {};
    deviceIds.insert(id); parsed.devices.append(row);
  }
  return parsed;
}
QString statusMessage(const QString &status) {
  if (status == "no_adapter") return "No Bluetooth adapter is available.";
  if (status == "bluez_unavailable") return "BlueZ is unavailable.";
  if (status == "system_bus_unavailable") return "The system D-Bus is unavailable.";
  if (status == "access_denied") return "Access to the read-only BlueZ inventory was denied.";
  if (status == "timeout") return "BlueZ did not return a snapshot before the deadline.";
  if (status == "unsupported_platform") return "Known-device inventory is unsupported on this platform.";
  if (status == "malformed_data") return "BlueZ returned malformed inventory data.";
  return {};
}
} // namespace

DeviceInventoryProvider::DeviceInventoryProvider(Options options, QObject *parent) : QObject(parent), m_options(std::move(options)), m_devices(this) {
  m_deadline.setSingleShot(true);
  m_age.setSingleShot(true);
  connect(&m_deadline, &QTimer::timeout, this, [this] { if (m_process) fail("timeout", "Inventory refresh exceeded the three second deadline."); });
  connect(&m_age, &QTimer::timeout, this, [this] {
    if (!m_updated.isValid() || loading()) return;
    m_devices.markStale();
    m_state = "stale";
    m_error = "Snapshot is over five minutes old. Refresh to load a new snapshot.";
    emit changed();
  });
}
DeviceInventoryProvider::~DeviceInventoryProvider() { retire(); }
QString DeviceInventoryProvider::lastUpdated() const { return m_updated.isValid() ? m_updated.toLocalTime().toString(Qt::ISODate) : QString(); }
bool DeviceInventoryProvider::current(QProcess *p, quint64 g) const { return p == m_process && g == m_generation; }
void DeviceInventoryProvider::retire() {
  ++m_generation; m_deadline.stop(); if (!m_process) return;
  auto *p = m_process.data(); m_process = nullptr; p->disconnect(this);
  if (p->state() == QProcess::NotRunning) p->deleteLater();
  else { connect(p, qOverload<int, QProcess::ExitStatus>(&QProcess::finished), p, &QObject::deleteLater); p->kill(); }
}
void DeviceInventoryProvider::fail(const QString &state, const QString &message) {
  retire(); m_state = m_updated.isValid() ? "stale" : state; m_status = state; m_error = message;
  if (m_updated.isValid()) m_devices.markStale();
  emit changed();
}
void DeviceInventoryProvider::cancel() { if (m_process) fail("cancelled", "Refresh cancelled. The previous snapshot is stale."); }
void DeviceInventoryProvider::read(QProcess *p, quint64 g, bool stderrChannel) {
  if (!current(p, g))
    return;
  p->setReadChannel(stderrChannel ? QProcess::StandardError : QProcess::StandardOutput);
  auto &buffer = stderrChannel ? m_stderr : m_stdout; const qsizetype maximum = stderrChannel ? MaxStderr : MaxOutput;
  buffer += p->read(maximum - buffer.size() + 1); if (buffer.size() > maximum) fail("invalid_report", "CLI output exceeded the inventory size limit.");
}
void DeviceInventoryProvider::refresh() {
  if (loading()) return;
  m_age.stop();
  const QFileInfo file(m_options.executable);
  if (!file.isAbsolute() || !file.isFile() || !file.isExecutable()) { fail("cli_unavailable", "CLI unavailable. Install opencontinuity beside this app or launch with --cli /absolute/path/opencontinuity."); return; }
  m_stdout.clear(); m_stderr.clear(); m_error.clear(); m_state = "loading"; m_status = "loading"; ++m_generation;
  auto *p = new QProcess(this); m_process = p; const auto generation = m_generation;
  p->setProgram(file.absoluteFilePath()); p->setArguments({"devices", "list", "--json"}); p->setProcessChannelMode(QProcess::SeparateChannels);
  connect(p,&QProcess::readyReadStandardOutput,this,[this,p,generation]{read(p,generation,false);});
  connect(p,&QProcess::readyReadStandardError,this,[this,p,generation]{read(p,generation,true);});
  connect(p,&QProcess::errorOccurred,this,[this,p,generation](QProcess::ProcessError e){if(current(p,generation)) fail(e==QProcess::FailedToStart?"cli_unavailable":"process_error","CLI could not complete the inventory. Check the executable and retry.");});
  connect(p,qOverload<int,QProcess::ExitStatus>(&QProcess::finished),this,[this,p,generation](int code,QProcess::ExitStatus exit){
    if (!current(p, generation))
      return;
    read(p, generation, false);
    read(p, generation, true);
    if (!current(p, generation))
      return;
    const auto parsed = m_stderr.isEmpty() ? parse(m_stdout) : std::optional<Parsed>{};
    if (!parsed || exit != QProcess::NormalExit ||
        (parsed->status == "ok" ? code != 0 : code == 0)) {
      fail("invalid_report", "CLI returned an invalid inventory schema or exit status.");
      return;
    }
    retire(); m_devices.replace(parsed->devices); m_adapters=parsed->adapters; m_status=parsed->status; m_state=parsed->status=="ok" ? "loaded" : parsed->status; m_error=statusMessage(parsed->status); m_updated=QDateTime::currentDateTimeUtc(); m_age.start(300000); emit changed();
  });
  m_deadline.start(m_options.timeoutMs); p->start(QIODevice::ReadOnly); emit changed();
}
