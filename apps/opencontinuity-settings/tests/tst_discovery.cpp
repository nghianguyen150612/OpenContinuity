#include "DeviceDiscoveryProvider.h"
#include "DeviceDiscoveryReport.h"
#include <QJsonDocument>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTest>

static QString fixtureExecutable;
static const QString sessionId = "0123456789abcdef0123456789abcdef";

static QJsonObject envelope(const QString &type, int sequence) {
  return {{"schema_version", 1}, {"source", "bluez_le_discovery_events"},
          {"type", type}, {"authorization", "never_evaluated"},
          {"scan_session_id", sessionId}, {"sequence", sequence}};
}
static QByteArray line(const QJsonObject &object) {
  return QJsonDocument(object).toJson(QJsonDocument::Compact);
}
static QJsonObject sessionEvent() {
  auto event = envelope("session", 1);
  event.insert("state", "started"); event.insert("status", "started");
  event.insert("scan_started", true); event.insert("requested_duration_seconds", 10);
  event.insert("adapter_index", 1); return event;
}
static QJsonObject deviceEvent(int sequence, const QString &name, const QString &change) {
  auto event = envelope("device", sequence);
  event.insert("id", "scan-0123456789abcdef-001"); event.insert("display_name", name);
  event.insert("provenance", "bluez_event_during_session"); event.insert("change", change);
  return event;
}
static QJsonObject terminalEvent(int sequence, int deviceEvents = 2) {
  auto event = envelope("terminal", sequence);
  event.insert("scan_requested", true); event.insert("scan_started", true);
  event.insert("scan_completed", true); event.insert("requested_duration_seconds", 10);
  event.insert("actual_duration_ms", 10000); event.insert("adapter_index", 1);
  event.insert("status", "completed"); event.insert("cleanup_status", "complete");
  event.insert("device_count", 1); event.insert("device_event_count", deviceEvents);
  event.insert("termination_reason", "duration_elapsed"); return event;
}

class DiscoveryTest final : public QObject {
  Q_OBJECT
private slots:
  void init() { qunsetenv("OC_SETTINGS_TEST_MODE"); qputenv("OC_DISCOVERY_MODE", "sample"); }
  void incrementalParserUpdatesModelBeforeTerminal() {
    DeviceDiscoveryReport report; DeviceDiscoveryModel model;
    QVERIFY(report.consumeLine(line(sessionEvent()), model));
    QVERIFY(report.consumeLine(line(deviceEvent(2, QString::fromUtf8("Watch 🎧"), "observed")), model));
    QCOMPARE(model.count(), 1); QCOMPARE(model.displayName("scan-0123456789abcdef-001"), QString::fromUtf8("Watch 🎧"));
    QVERIFY(report.consumeLine(line(deviceEvent(3, "Renamed watch", "updated")), model));
    QCOMPARE(model.count(), 1); QCOMPARE(model.displayName("scan-0123456789abcdef-001"), "Renamed watch");
    QVERIFY(report.consumeLine(line(terminalEvent(4)), model));
    QVERIFY(report.finish(0, true, model));
    model.markStale(); QVERIFY(model.stale());
    model.clear(); QCOMPARE(model.count(), 0); QVERIFY(!model.stale());
  }
  void parserRejectsMalformedStreams_data() {
    QTest::addColumn<QString>("mode");
    for (const auto &mode : {"invalid", "truncated", "invalid_utf8", "duplicate",
                             "escaped_duplicate", "wrong_schema", "wrong_session",
                             "bad_sequence", "bad_terminal", "exit_mismatch",
                             "no_terminal", "duplicate_update"})
      QTest::newRow(mode) << QString(mode);
  }
  void parserRejectsMalformedStreams() {
    QFETCH(QString, mode); qputenv("OC_DISCOVERY_MODE", mode.toUtf8());
    DeviceDiscoveryProvider provider({fixtureExecutable}); provider.scan();
    QTRY_VERIFY_WITH_TIMEOUT(!provider.scanning(), 5000);
    QCOMPARE(provider.state(), "failed"); QCOMPARE(provider.cleanupStatus(), "incomplete");
    QVERIFY(provider.devices()->stale());
  }
  void fragmentedUtf8AndCoalescedMessages() {
    for (const auto &mode : {"fragmented_unicode", "coalesced"}) {
      qputenv("OC_DISCOVERY_MODE", mode); DeviceDiscoveryProvider provider({fixtureExecutable}); provider.scan();
      QTRY_COMPARE_WITH_TIMEOUT(provider.state(), "completed", 5000);
      QCOMPARE(provider.devices()->count(), 1);
      if (QString(mode) == "fragmented_unicode")
        QCOMPARE(provider.devices()->displayName("scan-0123456789abcdef-001"), QString::fromUtf8("Watch 🎧"));
    }
  }
  void liveModelUpdateArrivesBeforeTerminal() {
    qputenv("OC_DISCOVERY_MODE", "live_update"); DeviceDiscoveryProvider provider({fixtureExecutable}); provider.scan();
    QTRY_VERIFY_WITH_TIMEOUT(provider.scanning() && provider.devices()->count() == 1, 5000);
    QCOMPARE(provider.devices()->displayName("scan-0123456789abcdef-001"), "Watch");
    QTRY_COMPARE_WITH_TIMEOUT(provider.devices()->displayName("scan-0123456789abcdef-001"), "Renamed watch", 5000);
    QVERIFY(provider.scanning());
    QTRY_COMPARE_WITH_TIMEOUT(provider.state(), "completed", 5000);
    QCOMPARE(provider.devices()->count(), 1);
  }
  void noAutomaticScanAndOneProcess() {
    DeviceDiscoveryProvider p({fixtureExecutable}); QCOMPARE(p.state(), "idle"); QVERIFY(!p.scanning()); QVERIFY(!p.durationKnown());
    QTest::qWait(150); QVERIFY(!p.scanning()); QCOMPARE(p.devices()->count(), 0);
    p.scan(); QVERIFY(p.scanning()); p.scan();
    QTRY_COMPARE_WITH_TIMEOUT(p.state(), "completed", 5000); QVERIFY(!p.scanning()); QCOMPARE(p.cleanupStatus(), "complete"); QCOMPARE(p.devices()->count(), 1); QVERIFY(p.durationKnown());
  }
  void cancellationWaitsForTerminalAndKeepsPartialResultsStale() {
    DeviceDiscoveryProvider p({fixtureExecutable}); p.scan(); QTRY_COMPARE_WITH_TIMEOUT(p.state(), "completed", 5000);
    qputenv("OC_DISCOVERY_MODE", "cancel"); p.scan(); QTest::qWait(100);
    p.cancel(); QVERIFY(p.cancelling()); QVERIFY(p.scanning());
    QTRY_COMPARE_WITH_TIMEOUT(p.state(), "cancelled", 5000); QCOMPARE(p.cleanupStatus(), "complete"); QVERIFY(p.devices()->stale());
    qputenv("OC_DISCOVERY_MODE", "empty"); p.scan(); QTRY_COMPARE_WITH_TIMEOUT(p.state(), "completed", 5000);
    QCOMPARE(p.devices()->count(), 0); QVERIFY(p.message().contains("No observations"));
    QTest::qWait(200); QCOMPARE(p.state(), "completed"); QVERIFY(!p.scanning());
  }
  void unavailableAndErrors_data() {
    QTest::addColumn<QString>("mode");
    for (const auto &mode : {"no_adapter", "adapter_not_powered", "access_denied", "unsupported_le", "adapter_busy", "bluez_unavailable", "system_bus_unavailable", "timeout", "malformed_data", "data_limit", "unsupported_platform", "cleanup_incomplete"}) QTest::newRow(mode) << QString(mode);
  }
  void unavailableAndErrors() {
    QFETCH(QString, mode); qputenv("OC_DISCOVERY_MODE", mode.toUtf8()); DeviceDiscoveryProvider p({fixtureExecutable}); p.scan();
    QTRY_COMPARE_WITH_TIMEOUT(p.state(), mode, 5000); QVERIFY(!p.message().isEmpty()); QCOMPARE(p.cleanupStatus(), mode == "cleanup_incomplete" ? "incomplete" : "not_needed");
  }
  void rejectsProcessOutput_data() { QTest::addColumn<QString>("mode"); for (const auto &m : {"oversized", "stderr"}) QTest::newRow(m) << QString(m); }
  void rejectsProcessOutput() {
    QFETCH(QString, mode); qputenv("OC_DISCOVERY_MODE", mode.toUtf8()); DeviceDiscoveryProvider p({fixtureExecutable}); p.scan(); QTRY_VERIFY_WITH_TIMEOUT(!p.scanning(), 5000); QCOMPARE(p.state(), "failed"); QCOMPARE(p.cleanupStatus(), "incomplete"); QVERIFY(!p.durationKnown());
  }
  void missingExecutable() { DeviceDiscoveryProvider p({"relative-cli"}); p.scan(); QCOMPARE(p.state(), "cli_unavailable"); QVERIFY(!p.scanning()); }
  void timeoutAndForcedTerminationAreUncertain() {
    qputenv("OC_DISCOVERY_MODE", "hang"); DeviceDiscoveryProvider p({fixtureExecutable, 250, 100});
    p.scan(); QTRY_VERIFY_WITH_TIMEOUT(!p.scanning(), 5000); QCOMPARE(p.state(), "failed"); QCOMPARE(p.cleanupStatus(), "incomplete"); QVERIFY(!p.durationKnown());
    qputenv("OC_DISCOVERY_MODE", "sample"); p.scan(); QTRY_COMPARE_WITH_TIMEOUT(p.state(), "completed", 5000); QTest::qWait(250); QCOMPARE(p.state(), "completed");
  }
  void shutdownConsumesTerminalCleanupReport() {
    qputenv("OC_DISCOVERY_MODE", "cancel"); DeviceDiscoveryProvider p({fixtureExecutable}); p.scan(); QTest::qWait(100);
    p.shutdown(); QVERIFY(!p.scanning()); QCOMPARE(p.state(), "cancelled"); QCOMPARE(p.cleanupStatus(), "complete");
  }
};

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv); if (argc < 2) return 2; fixtureExecutable = QString::fromLocal8Bit(argv[1]);
  DiscoveryTest test; return QTest::qExec(&test, argc, argv);
}
#include "tst_discovery.moc"
