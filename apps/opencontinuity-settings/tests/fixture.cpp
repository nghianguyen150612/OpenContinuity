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
  const bool discovery = command == QStringList{"devices", "discover", "--seconds", "10", "--events-jsonl"};
  if (!inventory && !discovery && command != QStringList{"doctor", "--json"}) return 90;
  if (discovery) {
    std::signal(SIGTERM, onSignal); std::signal(SIGINT, onSignal);
    const auto mode = qEnvironmentVariable("OC_DISCOVERY_MODE", "sample");
    if (mode == "invalid") { std::fputs("{invalid}\n", stdout); return 0; }
    if (mode == "truncated") { std::fputs("{\"schema_version\":1", stdout); return 0; }
    if (mode == "invalid_utf8") { const char bytes[] = {'{', '"', 0xff, '"', '}', '\n'}; std::fwrite(bytes, 1, sizeof(bytes), stdout); return 0; }
    if (mode == "oversized") { const QByteArray bytes(700 * 1024, 'x'); std::fwrite(bytes.data(), 1, bytes.size(), stdout); return 0; }
    if (mode == "stderr") std::fputs("unexpected private diagnostic", stderr);
    const QString sessionId = "0123456789abcdef0123456789abcdef";
    const bool started = !QStringList{"no_adapter", "adapter_not_powered", "access_denied", "unsupported_le", "adapter_busy", "bluez_unavailable", "system_bus_unavailable", "timeout", "malformed_data", "data_limit", "unsupported_platform"}.contains(mode);
    auto session = [&](int sequence, bool isStarted, const QString &status) {
      QJsonObject event{{"schema_version", 1}, {"source", "bluez_le_discovery_events"},
                        {"type", "session"}, {"authorization", "never_evaluated"},
                        {"scan_session_id", sessionId}, {"sequence", sequence},
                        {"state", isStarted ? "started" : "not_started"},
                        {"status", isStarted ? "started" : status},
                        {"scan_started", isStarted}, {"requested_duration_seconds", 10},
                        {"adapter_index", isStarted || mode == "unsupported_le" ? QJsonValue(1) : QJsonValue(QJsonValue::Null)}};
      return event;
    };
    auto device = [&](int sequence, const QString &name, const QString &change = "observed") {
      return QJsonObject{{"schema_version", 1}, {"source", "bluez_le_discovery_events"},
                         {"type", "device"}, {"authorization", "never_evaluated"},
                         {"scan_session_id", sessionId}, {"sequence", sequence},
                         {"id", "scan-0123456789abcdef-001"}, {"display_name", name},
                         {"provenance", "bluez_event_during_session"}, {"change", change}};
    };
    auto terminal = [&](int sequence, const QString &status, const QString &cleanup,
                        bool isStarted, int count, int eventCount, bool completed,
                        int actualMs) {
      QString reason = status == "completed" ? "duration_elapsed" : status == "cancelled" ? "user_cancelled" :
          status == "no_adapter" ? "adapter_lost_or_unavailable" : status == "adapter_not_powered" ? "adapter_power_lost_or_off" :
          status == "bluez_unavailable" ? "bluez_owner_lost_or_unavailable" : status == "timeout" ? "deadline_expired" :
          status == "data_limit" ? "stream_or_input_limit" : status == "output_blocked" ? "stdout_blocked" :
          status == "output_closed" ? "stdout_closed" : status == "cleanup_incomplete" ? "cleanup_failed" : "setup_or_observation_failed";
      return QJsonObject{{"schema_version", 1}, {"source", "bluez_le_discovery_events"},
                         {"type", "terminal"}, {"authorization", "never_evaluated"},
                         {"scan_session_id", sessionId}, {"sequence", sequence},
                         {"scan_requested", true}, {"scan_started", isStarted},
                         {"scan_completed", completed}, {"requested_duration_seconds", 10},
                         {"actual_duration_ms", actualMs},
                         {"adapter_index", isStarted || mode == "unsupported_le" ? QJsonValue(1) : QJsonValue(QJsonValue::Null)},
                         {"status", status}, {"cleanup_status", cleanup},
                         {"device_count", count}, {"device_event_count", eventCount},
                         {"termination_reason", reason}};
    };
    auto write = [](const QJsonObject &event) {
      auto bytes = QJsonDocument(event).toJson(QJsonDocument::Compact) + "\n";
      std::fwrite(bytes.data(), 1, bytes.size(), stdout); std::fflush(stdout);
    };
    if (mode == "duplicate") { std::fputs("{\"schema_version\":1,\"schema_version\":1}\n", stdout); return 0; }
    auto first = session(1, started, started ? QString("started") : mode);
    if (mode == "wrong_schema") first.insert("schema_version", 2);
    if (mode == "bad_sequence") first.insert("sequence", 2);
    if (mode == "escaped_duplicate") { std::fputs("{\"schema_version\":1,\"schema_\\u0076ersion\":1}\n", stdout); return 0; }
    write(first);
    if (!started) {
      auto final = terminal(2, mode, "not_needed", false, 0, 0, false, 0);
      write(final); return 1;
    }
    if (mode == "hang") { while (true) QThread::msleep(10); }
    if (mode == "cancel") {
      while (!cancelled) QThread::msleep(10);
      write(terminal(2, "cancelled", "complete", true, 0, 0, false, 50));
      return 1;
    }
    if (mode == "cleanup_incomplete") {
      write(terminal(2, mode, "incomplete", true, 0, 0, false, 120)); return 1;
    }
    if (mode == "empty") {
      write(terminal(2, "completed", "complete", true, 0, 0, true, 10000)); return 0;
    }
    if (mode == "no_terminal") { QThread::msleep(50); return 0; }
    if (mode == "wrong_session") {
      auto bad = device(2, "Watch"); bad.insert("scan_session_id", "ffffffffffffffffffffffffffffffff"); write(bad); return 1;
    }
    if (mode == "bad_terminal" || mode == "exit_mismatch" || mode == "coalesced" || mode == "live_update" || mode == "fragmented_unicode") {
      const QString name = mode == "fragmented_unicode" ? QString::fromUtf8("Watch 🎧") : "Watch";
      auto observed = device(2, name);
      auto updated = device(3, "Renamed watch", "updated");
      auto final = terminal(mode == "live_update" ? 4 : mode == "bad_terminal" ? 3 : mode == "fragmented_unicode" ? 3 : 3,
                            "completed", "complete", true, mode == "bad_terminal" ? 0 : 1,
                            mode == "live_update" ? 2 : 1, true, 10000);
      if (mode == "coalesced") {
        const QByteArray bytes = QJsonDocument(observed).toJson(QJsonDocument::Compact) + "\n" +
            QJsonDocument(final).toJson(QJsonDocument::Compact) + "\n";
        std::fwrite(bytes.data(), 1, bytes.size(), stdout); std::fflush(stdout); return 0;
      }
      if (mode == "fragmented_unicode") {
        auto bytes = QJsonDocument(observed).toJson(QJsonDocument::Compact) + "\n";
        const auto split = bytes.indexOf("🎧") + 2;
        std::fwrite(bytes.data(), 1, split, stdout); std::fflush(stdout); QThread::msleep(20);
        std::fwrite(bytes.data() + split, 1, bytes.size() - split, stdout); std::fflush(stdout);
      } else {
        write(observed);
      }
      if (mode == "live_update") { QThread::msleep(180); write(updated); QThread::msleep(180); }
      write(final);
      return mode == "exit_mismatch" ? 1 : 0;
    }
    if (mode == "duplicate_update") { auto observed = device(2, "Watch"); write(observed); auto update = device(3, "Watch", "updated"); write(update); return 0; }
    auto observed = device(2, "Nearby device"); write(observed);
    write(terminal(3, "completed", "complete", true, 1, 1, true, 10000));
    return 0;
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
  if (mode == "crash") {
#ifdef Q_OS_UNIX
    // A non-core-dumping fatal signal still gives QProcess CrashExit, without
    // waiting for an OS core collector before the parent can reap the child.
    std::raise(SIGKILL);
#else
    std::abort();
#endif
  }
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
