#include "DeviceInventoryProvider.h"
#include <QCoreApplication>
#include <QProcessEnvironment>
#include <QSignalSpy>
#include <QTest>

class InventoryTest final : public QObject {
  Q_OBJECT
private slots:
  void providerAcceptsSnapshot() {
    qputenv("OC_INVENTORY_MODE", "sample");
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1)});
    QSignalSpy changes(&provider, &DeviceInventoryProvider::changed);
    QSignalSpy modelChanges(provider.devices(), &QAbstractItemModel::modelReset);
    provider.refresh();
    QTRY_COMPARE(provider.state(), "loaded");
    QCOMPARE(provider.status(), "ok");
    QCOMPARE(provider.adapterCount(), 1);
    QCOMPARE(provider.devices()->count(), 1);
    const auto index = provider.devices()->index(0);
    QCOMPARE(provider.devices()->data(index, DeviceInventoryModel::DisplayNameRole).toString(), "Headphones");
    QCOMPARE(provider.devices()->data(index, DeviceInventoryModel::ConnectedRole).toBool(), false);
    QVERIFY(!changes.isEmpty());
    QCOMPARE(modelChanges.size(), 1);
  }
  void emptyIsNotUnavailable() {
    qputenv("OC_INVENTORY_MODE", "empty");
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1)});
    provider.refresh(); QTRY_COMPARE(provider.state(), "loaded");
    QCOMPARE(provider.devices()->count(), 0); QCOMPARE(provider.status(), "ok");
    QCOMPARE(provider.adapterCount(), 1);
  }
  void noAdapterIsExplicit() {
    qputenv("OC_INVENTORY_MODE", "no_adapter");
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1)});
    provider.refresh(); QTRY_COMPARE(provider.state(), "no_adapter");
    QCOMPARE(provider.status(), "no_adapter"); QCOMPARE(provider.devices()->count(), 0);
  }
  void statusOutcomesRemainDistinct_data() {
    QTest::addColumn<QString>("status");
    for (const auto &status : {"access_denied", "bluez_unavailable", "system_bus_unavailable", "timeout", "malformed_data", "unsupported_platform"})
      QTest::newRow(status) << QString(status);
  }
  void statusOutcomesRemainDistinct() {
    QFETCH(QString, status); qputenv("OC_INVENTORY_MODE", status.toUtf8());
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1)});
    provider.refresh(); QTRY_COMPARE(provider.state(), status);
    QCOMPARE(provider.status(), status); QVERIFY(!provider.lastError().isEmpty());
    QCOMPARE(provider.devices()->count(), 0);
  }
  void missingCliIsExplicit() {
    DeviceInventoryProvider provider({"/nonexistent/opencontinuity"}); provider.refresh();
    QCOMPARE(provider.state(), "cli_unavailable"); QCOMPARE(provider.devices()->count(), 0);
  }
  void rejectsInvalidInventorySchemas_data() {
    QTest::addColumn<QString>("mode");
    for (const auto &mode : {"malformed", "unknown_schema", "missing_field", "duplicate_field", "invalid_bool", "duplicate_ids", "invalid_status", "invalid_association", "oversized"})
      QTest::newRow(mode) << QString(mode);
  }
  void rejectsInvalidInventorySchemas() {
    QFETCH(QString, mode);
    qputenv("OC_INVENTORY_MODE", mode.toUtf8());
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1)});
    provider.refresh(); QTRY_COMPARE(provider.state(), "invalid_report");
    QCOMPARE(provider.devices()->count(), 0);
  }
  void timeoutCancellationRejectsLateResult() {
    qunsetenv("OC_INVENTORY_MODE"); qputenv("OC_SETTINGS_TEST_MODE", "timeout");
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1), 1000});
    provider.refresh(); QTRY_COMPARE(provider.state(), "timeout");
    provider.cancel(); QTest::qWait(250);
    QCOMPARE(provider.state(), "timeout"); QCOMPARE(provider.devices()->count(), 0);
    qunsetenv("OC_SETTINGS_TEST_MODE");
  }
  void nonzeroExitIsRejected() {
    qputenv("OC_SETTINGS_TEST_MODE", "nonzero");
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1)});
    provider.refresh(); QTRY_COMPARE(provider.state(), "invalid_report");
    QCOMPARE(provider.devices()->count(), 0); qunsetenv("OC_SETTINGS_TEST_MODE");
  }
  void excessiveStderrIsRejected() {
    qputenv("OC_SETTINGS_TEST_MODE", "stderr-large");
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1)});
    provider.refresh(); QTRY_COMPARE(provider.state(), "invalid_report");
    QCOMPARE(provider.devices()->count(), 0); qunsetenv("OC_SETTINGS_TEST_MODE");
  }
  void crashIsRejected() {
    qputenv("OC_SETTINGS_TEST_MODE", "crash");
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1)});
    provider.refresh(); QTRY_VERIFY(provider.state() == "process_error" || provider.state() == "invalid_report");
    QCOMPARE(provider.devices()->count(), 0); qunsetenv("OC_SETTINGS_TEST_MODE");
  }
  void cancellationRejectsLateCallback() {
    qputenv("OC_SETTINGS_TEST_MODE", "delay");
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1)});
    provider.refresh(); QVERIFY(provider.loading()); provider.cancel();
    QCOMPARE(provider.state(), "cancelled"); QTest::qWait(250);
    QCOMPARE(provider.state(), "cancelled"); QCOMPARE(provider.devices()->count(), 0);
    qunsetenv("OC_SETTINGS_TEST_MODE");
  }
  void failedRefreshMarksPreviousRowsStale() {
    qputenv("OC_INVENTORY_MODE", "sample"); qunsetenv("OC_SETTINGS_TEST_MODE");
    DeviceInventoryProvider provider({QCoreApplication::arguments().at(1), 1000});
    provider.refresh(); QTRY_COMPARE(provider.state(), "loaded");
    qputenv("OC_SETTINGS_TEST_MODE", "timeout");
    provider.refresh(); QTRY_COMPARE(provider.state(), "stale");
    QVERIFY(provider.devices()->stale());
    QVERIFY(provider.devices()->data(provider.devices()->index(0), DeviceInventoryModel::StaleRole).toBool());
    QCOMPARE(provider.devices()->data(provider.devices()->index(0), DeviceInventoryModel::ConnectedRole).toBool(), false);
    qunsetenv("OC_SETTINGS_TEST_MODE");
  }
};
QTEST_GUILESS_MAIN(InventoryTest)
#include "tst_inventory.moc"
