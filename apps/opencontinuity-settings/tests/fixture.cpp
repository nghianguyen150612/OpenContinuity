// Controlled native test peer. Never interprets commands, arguments or shell
// text.
#include <QCoreApplication>
#include <QFile>
#include <QThread>
#include <cstdio>
#include <csignal>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
namespace { volatile std::sig_atomic_t cancelled = 0; void onSignal(int) { cancelled = 1; } }
int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  const auto command = app.arguments().mid(1);
  const bool inventory = command == QStringList{"devices", "list", "--json"};
  const bool discovery = command == QStringList{"devices", "discover", "--seconds", "10", "--json"};
  if (!inventory && !discovery && command != QStringList{"doctor", "--json"}) return 90;
  if (discovery) {
    std::signal(SIGTERM, onSignal); std::signal(SIGINT, onSignal);
    const auto mode = qEnvironmentVariable("OC_DISCOVERY_MODE", "sample");
    const int duration = mode == "cancel" || mode == "hang" ? 2000 : 120;
    for (int i = 0; i < duration && (!cancelled || mode == "hang"); i += 10) QThread::msleep(10);
    if (mode == "invalid") { std::fputs("{invalid", stdout); return 0; }
    if (mode == "oversized") { const QByteArray bytes(70000, 'x'); std::fwrite(bytes.data(), 1, bytes.size(), stdout); return 0; }
    if (mode == "stderr") std::fputs("unexpected private diagnostic", stderr);
    QFile file(":/device-discovery.json"); if (!file.open(QIODevice::ReadOnly)) return 93;
    auto report = QJsonDocument::fromJson(file.readAll()).object();
    if (mode == "empty") report.insert("devices", QJsonArray{});
    if (cancelled && mode != "hang") {
      report.insert("status", "cancelled"); report.insert("scan_completed", false); report.insert("actual_duration_ms", 50);
    } else if (QStringList{"no_adapter", "adapter_not_powered", "access_denied", "unsupported_le", "adapter_busy", "bluez_unavailable", "system_bus_unavailable", "timeout", "malformed_data", "data_limit", "unsupported_platform"}.contains(mode)) {
      report.insert("status", mode); report.insert("scan_started", false); report.insert("scan_completed", false);
      report.insert("actual_duration_ms", 0); report.insert("adapter_index", QJsonValue::Null); report.insert("devices", QJsonArray{}); report.insert("cleanup_status", "not_needed");
    } else if (mode == "cleanup_incomplete") { report.insert("status", mode); report.insert("scan_completed", false); report.insert("cleanup_status", "incomplete"); }
    const auto bytes = QJsonDocument(report).toJson(QJsonDocument::Compact);
    std::fwrite(bytes.data(), 1, bytes.size(), stdout); std::fflush(stdout);
    return report.value("status").toString() == "completed" ? 0 : 1;
  }
  const auto mode = qEnvironmentVariable("OC_SETTINGS_TEST_MODE");
  if (mode == "timeout")
    QThread::msleep(2000);
  if (mode == "delay")
    QThread::msleep(200);
  if (mode == "nonzero") {
    std::fputs("<b>untrusted error</b>", stderr);
    return 7;
  }
  if (mode == "crash")
    std::abort();
  if (mode == "oversized") {
    const QByteArray large(20000, 'x');
    std::fwrite(large.data(), 1, large.size(), stdout);
    return 0;
  }
  if (mode == "stderr-large") {
    const QByteArray large(5000, 'x');
    std::fwrite(large.data(), 1, large.size(), stderr);
    return 0;
  }
  if (mode == "stderr")
    std::fputs("unexpected diagnostic", stderr);
  if (mode == "invalid") {
    std::fputs("{invalid", stdout);
    return 0;
  }
  if (inventory) {
    const auto inventoryMode = qEnvironmentVariable("OC_INVENTORY_MODE", "sample");
    if (inventoryMode == "malformed") { std::fputs("{truncated", stdout); return 0; }
    if (inventoryMode == "invalid_status") { std::fputs(R"({"schema_version":1,"source":"bluez_known_device_snapshot","scan_performed":false,"status":"success","adapters":[],"devices":[],"authorization":"never_evaluated"})", stdout); return 0; }
    if (inventoryMode == "invalid_association") { std::fputs(R"({"schema_version":1,"source":"bluez_known_device_snapshot","scan_performed":false,"status":"ok","adapters":[{"id":"adapter-001","powered":true}],"devices":[{"id":"device-001","adapter_id":"adapter-999","display_name":"One","connected":false,"paired":true,"trusted":null,"services_resolved":null}],"authorization":"never_evaluated"})", stdout); return 0; }
    if (inventoryMode == "unknown_schema") { std::fputs(R"({"schema_version":2,"source":"bluez_known_device_snapshot","scan_performed":false,"status":"ok","adapters":[],"devices":[],"authorization":"never_evaluated"})", stdout); return 0; }
    if (inventoryMode == "missing_field") { std::fputs(R"({"schema_version":1,"source":"bluez_known_device_snapshot","scan_performed":false,"status":"ok","adapters":[],"devices":[]})", stdout); return 0; }
    if (inventoryMode == "duplicate_field") { std::fputs(R"({"schema_version":1,"schema_version":1,"source":"bluez_known_device_snapshot","scan_performed":false,"status":"ok","adapters":[],"devices":[],"authorization":"never_evaluated"})", stdout); return 0; }
    if (inventoryMode == "invalid_bool") { std::fputs(R"({"schema_version":1,"source":"bluez_known_device_snapshot","scan_performed":false,"status":"ok","adapters":[{"id":"adapter-001","powered":"yes"}],"devices":[],"authorization":"never_evaluated"})", stdout); return 0; }
    if (inventoryMode == "duplicate_ids") { std::fputs(R"({"schema_version":1,"source":"bluez_known_device_snapshot","scan_performed":false,"status":"ok","adapters":[{"id":"adapter-001","powered":true}],"devices":[{"id":"device-001","adapter_id":"adapter-001","display_name":"One","connected":false,"paired":true,"trusted":null,"services_resolved":null},{"id":"device-001","adapter_id":"adapter-001","display_name":"Two","connected":false,"paired":true,"trusted":null,"services_resolved":null}],"authorization":"never_evaluated"})", stdout); return 0; }
    if (inventoryMode == "access_denied" || inventoryMode == "bluez_unavailable" || inventoryMode == "system_bus_unavailable" || inventoryMode == "timeout" || inventoryMode == "malformed_data" || inventoryMode == "unsupported_platform") {
      const auto bytes = QString(R"({"schema_version":1,"source":"bluez_known_device_snapshot","scan_performed":false,"status":"%1","adapters":[],"devices":[],"authorization":"never_evaluated"})").arg(inventoryMode).toUtf8();
      std::fwrite(bytes.data(), 1, bytes.size(), stdout); return 1;
    }
    if (inventoryMode == "oversized") { const QByteArray large(70 * 1024, 'x'); std::fwrite(large.data(), 1, large.size(), stdout); return 0; }
    if (inventoryMode == "empty") {
      const QByteArray bytes = R"({"schema_version":1,"source":"bluez_known_device_snapshot","scan_performed":false,"status":"ok","adapters":[{"id":"adapter-001","powered":null}],"devices":[],"authorization":"never_evaluated"})";
      std::fwrite(bytes.data(), 1, bytes.size(), stdout); return 0;
    }
    if (inventoryMode == "no_adapter") {
      const QByteArray bytes = R"({"schema_version":1,"source":"bluez_known_device_snapshot","scan_performed":false,"status":"no_adapter","adapters":[],"devices":[],"authorization":"never_evaluated"})";
      std::fwrite(bytes.data(), 1, bytes.size(), stdout); return 1;
    }
    QFile file(":/device-inventory.json");
    if (!file.open(QIODevice::ReadOnly)) return 92;
    const auto bytes = file.readAll(); std::fwrite(bytes.data(), 1, bytes.size(), stdout); return 0;
  }
  QFile file(":/report.json");
  if (!file.open(QIODevice::ReadOnly))
    return 91;
  const auto bytes = file.readAll();
  std::fwrite(bytes.data(), 1, bytes.size(), stdout);
  return 0;
}
