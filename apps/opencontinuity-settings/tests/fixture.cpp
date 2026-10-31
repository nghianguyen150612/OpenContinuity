// Controlled native test peer. Never interprets commands, arguments or shell
// text.
#include <QCoreApplication>
#include <QFile>
#include <QThread>
#include <cstdio>
int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  const auto command = app.arguments().mid(1);
  const bool inventory = command == QStringList{"devices", "list", "--json"};
  if (!inventory && command != QStringList{"doctor", "--json"}) return 90;
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
