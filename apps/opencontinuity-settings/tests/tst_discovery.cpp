#include "DeviceDiscoveryProvider.h"
#include "DeviceDiscoveryReport.h"
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTest>
static QString fixtureExecutable;
static QByteArray sample() { QFile f(":/device-discovery.json"); if (!f.open(QIODevice::ReadOnly)) return {}; return f.readAll(); }
class DiscoveryTest final : public QObject {
  Q_OBJECT
private slots:
  void init() { qunsetenv("OC_SETTINGS_TEST_MODE"); qputenv("OC_DISCOVERY_MODE", "sample"); }
  void parserAndModel() {
    const auto report = DeviceDiscoveryReport::parse(sample()); QVERIFY(report); QCOMPARE(report->devices.size(), 1);
    DeviceDiscoveryModel model; QSignalSpy changes(&model, &DeviceDiscoveryModel::changed);
    model.replace(report->devices); QCOMPARE(model.count(), 1);
    QCOMPARE(model.data(model.index(0), DeviceDiscoveryModel::DisplayNameRole).toString(), "Test peripheral");
    QVERIFY(!model.data(model.index(-1), DeviceDiscoveryModel::DisplayNameRole).isValid());
    model.markStale(); QVERIFY(model.stale()); QVERIFY(model.data(model.index(0), DeviceDiscoveryModel::StaleRole).toBool());
    model.replace({}); QCOMPARE(model.count(), 0); QVERIFY(!model.stale()); QVERIFY(changes.count() >= 3);
  }
  void malformedSchemas_data() {
    QTest::addColumn<QByteArray>("input");
    QTest::newRow("invalid") << QByteArray("{invalid"); QTest::newRow("large") << QByteArray(65537, 'x');
    QTest::newRow("duplicate") << QByteArray(sample()).replace("\"schema_version\":1", "\"schema_version\":1,\"schema_version\":1");
    QTest::newRow("escaped-duplicate") << QByteArray(sample()).replace("\"schema_version\":1", "\"schema_version\":1,\"schema_versio\\u006e\":1");
    for (const auto &[field,value] : QList<QPair<QString,QJsonValue>>{
         {"schema_version",2}, {"source","bluez_known_device_snapshot"}, {"scan_requested",false}, {"scan_started",false},
         {"scan_completed",false}, {"cleanup_status","not_needed"}, {"requested_duration_seconds",31}, {"actual_duration_ms",2},
         {"adapter_index",QJsonValue::Null}, {"authorization","authorized"}, {"status","success"}}) {
      auto object = QJsonDocument::fromJson(sample()).object(); object.insert(field,value);
      QTest::newRow(field.toUtf8().constData()) << QJsonDocument(object).toJson();
    }
    for (const auto &name : {"AA:BB:CC:DD:EE:FF", "/org/bluez/hci0", "bad\nname", ""}) {
      auto object = QJsonDocument::fromJson(sample()).object(); auto row = object.value("devices").toArray().first().toObject(); row.insert("display_name",name); object.insert("devices",QJsonArray{row});
      QTest::newRow(qPrintable(QString("name-%1").arg(name))) << QJsonDocument(object).toJson();
    }
    auto object = QJsonDocument::fromJson(sample()).object(); auto rows = object.value("devices").toArray(); rows.append(rows.first()); object.insert("devices", rows);
    QTest::newRow("duplicate-id") << QJsonDocument(object).toJson();
    object.remove("status"); QTest::newRow("missing-field") << QJsonDocument(object).toJson();
  }
  void malformedSchemas() { QFETCH(QByteArray,input); QVERIFY(!DeviceDiscoveryReport::parse(input)); }
  void noAutomaticScanAndOneProcess() {
    DeviceDiscoveryProvider p({fixtureExecutable}); QCOMPARE(p.state(), "idle"); QVERIFY(!p.scanning()); QVERIFY(!p.durationKnown());
    QTest::qWait(150); QVERIFY(!p.scanning()); QCOMPARE(p.devices()->count(), 0);
    p.scan(); QVERIFY(p.scanning()); p.scan();
    QTRY_COMPARE(p.state(), "completed"); QVERIFY(!p.scanning()); QCOMPARE(p.cleanupStatus(), "complete"); QCOMPARE(p.devices()->count(), 1); QVERIFY(p.durationKnown());
  }
  void cancellationWaitsForFinalReportAndPreviousResultsAreStale() {
    DeviceDiscoveryProvider p({fixtureExecutable}); p.scan(); QTRY_COMPARE(p.state(), "completed");
    qputenv("OC_DISCOVERY_MODE","cancel"); p.scan(); QVERIFY(p.devices()->stale()); QTest::qWait(100);
    p.cancel(); QVERIFY(p.cancelling()); QVERIFY(p.scanning());
    QTRY_COMPARE(p.state(), "cancelled"); QCOMPARE(p.cleanupStatus(), "complete"); QVERIFY(p.devices()->stale());
    qputenv("OC_DISCOVERY_MODE","empty"); p.scan(); QTRY_COMPARE(p.state(), "completed");
    QCOMPARE(p.devices()->count(),0); QVERIFY(p.message().contains("No observations"));
    QTest::qWait(200); QCOMPARE(p.state(),"completed"); QVERIFY(!p.scanning());
  }
  void unavailableAndErrors_data() {
    QTest::addColumn<QString>("mode");
    for (const auto &mode : {"no_adapter","adapter_not_powered","access_denied","unsupported_le","adapter_busy","bluez_unavailable","system_bus_unavailable","timeout","malformed_data","data_limit","unsupported_platform","cleanup_incomplete"}) QTest::newRow(mode) << QString(mode);
  }
  void unavailableAndErrors() {
    QFETCH(QString,mode); qputenv("OC_DISCOVERY_MODE",mode.toUtf8()); DeviceDiscoveryProvider p({fixtureExecutable}); p.scan();
    QTRY_COMPARE(p.state(),mode); QVERIFY(!p.message().isEmpty()); QCOMPARE(p.cleanupStatus(), mode == "cleanup_incomplete" ? "incomplete" : "not_needed");
  }
  void rejectsProcessOutput_data() { QTest::addColumn<QString>("mode"); for (const auto &m : {"invalid","oversized","stderr"}) QTest::newRow(m) << QString(m); }
  void rejectsProcessOutput() {
    QFETCH(QString,mode); qputenv("OC_DISCOVERY_MODE",mode.toUtf8()); DeviceDiscoveryProvider p({fixtureExecutable}); p.scan(); QTRY_VERIFY(!p.scanning()); QCOMPARE(p.state(),"failed"); QCOMPARE(p.cleanupStatus(),"incomplete"); QVERIFY(!p.durationKnown());
  }
  void missingExecutable() { DeviceDiscoveryProvider p({"relative-cli"}); p.scan(); QCOMPARE(p.state(),"cli_unavailable"); QVERIFY(!p.scanning()); }
  void timeoutAndForcedTerminationAreUncertain() {
    qputenv("OC_DISCOVERY_MODE","hang"); DeviceDiscoveryProvider p({fixtureExecutable, 250, 100});
    p.scan(); QTRY_VERIFY(!p.scanning()); QCOMPARE(p.state(),"failed"); QCOMPARE(p.cleanupStatus(),"incomplete"); QVERIFY(!p.durationKnown());
    qputenv("OC_DISCOVERY_MODE","sample"); p.scan(); QTRY_COMPARE(p.state(),"completed"); QTest::qWait(250); QCOMPARE(p.state(),"completed");
  }
  void shutdownRequestsCleanup() {
    qputenv("OC_DISCOVERY_MODE","cancel"); DeviceDiscoveryProvider p({fixtureExecutable}); p.scan(); QTest::qWait(100);
    p.shutdown(); QVERIFY(!p.scanning()); QCOMPARE(p.state(),"cancelled"); QCOMPARE(p.cleanupStatus(),"complete");
  }
};
int main(int argc, char **argv) {
  QCoreApplication app(argc,argv); if (argc < 2) return 2; fixtureExecutable = QString::fromLocal8Bit(argv[1]);
  DiscoveryTest test; int count = 1; return QTest::qExec(&test,count,argv);
}
#include "tst_discovery.moc"
