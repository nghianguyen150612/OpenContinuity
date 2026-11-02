#include "DoctorProvider.h"
#include "DeviceInventoryProvider.h"
#include "DeviceDiscoveryProvider.h"
#include <QGuiApplication>
#include <QPalette>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickItem>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QSignalSpy>
#include <QTest>

// ListView delegates have QML context ownership; follow the visual tree for
// clicks.
static QQuickItem *visualItem(QQuickItem *root, const QString &name) {
  if (root->objectName() == name)
    return root;
  for (auto *child : root->childItems())
    if (auto *found = visualItem(child, name))
      return found;
  return nullptr;
}

class UiTest : public QObject {
  Q_OBJECT
private slots:
  void realWindowAndNavigation_data() {
    QTest::addColumn<bool>("unavailable");
    QTest::addColumn<bool>("dark");
    QTest::addColumn<QString>("inventoryOutcome");
    QTest::addColumn<QString>("discoveryOutcome");
    QTest::newRow("loaded-light") << false << false << QString("sample") << QString("sample");
    QTest::newRow("live-observation-update") << false << false << QString("sample") << QString("live_update");
    QTest::newRow("loaded-dark") << false << true << QString("sample") << QString("sample");
    QTest::newRow("successful-empty-inventory") << false << false << QString("empty") << QString("empty");
    QTest::newRow("missing-bluez") << false << false << QString("bluez_unavailable") << QString("bluez_unavailable");
    QTest::newRow("cleanup-warning") << false << false << QString("sample") << QString("cleanup_incomplete");
    QTest::newRow("powered-off") << false << false << QString("sample") << QString("adapter_not_powered");
    QTest::newRow("missing-cli") << true << false << QString("sample") << QString("sample");
  }
  void realWindowAndNavigation() {
    QFETCH(bool, unavailable);
    QFETCH(bool, dark);
    QFETCH(QString, inventoryOutcome);
    QFETCH(QString, discoveryOutcome);
    QPalette palette;
    if (dark) {
      palette.setColor(QPalette::Window, QColor("#20232a"));
      palette.setColor(QPalette::WindowText, QColor("#f0f2f7"));
      palette.setColor(QPalette::Base, QColor("#282d36"));
      palette.setColor(QPalette::AlternateBase, QColor("#252932"));
      palette.setColor(QPalette::Text, QColor("#f0f2f7"));
      palette.setColor(QPalette::Button, QColor("#343b48"));
      palette.setColor(QPalette::ButtonText, QColor("#f0f2f7"));
      palette.setColor(QPalette::Mid, QColor("#667080"));
      palette.setColor(QPalette::Highlight, QColor("#3865cb"));
      palette.setColor(QPalette::HighlightedText, QColor("#ffffff"));
    }
    QGuiApplication::setPalette(palette);
    const auto realCli = qEnvironmentVariable("OC_REAL_CLI");
    const auto cli =
        unavailable ? QString("/nonexistent/opencontinuity")
        : realCli.isEmpty()
            ? QCoreApplication::applicationDirPath() + "/doctor-fixture"
            : realCli;
    DoctorProvider backend({cli});
    const auto inventoryCli = unavailable ? QString("/nonexistent/opencontinuity")
        : qEnvironmentVariable("OC_INVENTORY_CLI", QCoreApplication::applicationDirPath() + "/doctor-fixture");
    DeviceInventoryProvider inventory({inventoryCli});
    DeviceDiscoveryProvider discovery({inventoryCli});
    qmlRegisterUncreatableType<StatusModel>("OpenContinuity.Backend", 1, 0,
                                            "StatusModel", "Owned");
    qmlRegisterUncreatableType<DoctorProvider>("OpenContinuity.Backend", 1, 0,
                                               "DoctorProvider", "Owned");
    qmlRegisterUncreatableType<DeviceInventoryModel>("OpenContinuity.Backend", 1, 0,
                                                     "DeviceInventoryModel", "Owned");
    qmlRegisterUncreatableType<DeviceInventoryProvider>("OpenContinuity.Backend", 1, 0,
                                                        "DeviceInventoryProvider", "Owned");
    qmlRegisterUncreatableType<DeviceDiscoveryModel>("OpenContinuity.Backend",1,0,"DeviceDiscoveryModel","Owned");
    qmlRegisterUncreatableType<DeviceDiscoveryProvider>("OpenContinuity.Backend",1,0,"DeviceDiscoveryProvider","Owned");
    QQmlApplicationEngine engine;
    QSignalSpy warnings(&engine, &QQmlEngine::warnings);
    engine.rootContext()->setContextProperty("doctor", &backend);
    engine.rootContext()->setContextProperty("deviceInventory", &inventory);
    engine.rootContext()->setContextProperty("deviceDiscovery", &discovery);
    engine.load(QUrl("qrc:/qml/Main.qml"));
    QVERIFY2(!engine.rootObjects().isEmpty(),
             "The real application window must load");
    auto *window = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
    QVERIFY(window);
    QTRY_VERIFY(window->isVisible());
    backend.refresh();
    QTRY_COMPARE(backend.reportState(),
                 unavailable ? "cli_unavailable" : "loaded");
    auto *nav = window->findChild<QQuickItem *>("navigation");
    QVERIFY(nav);
    nav->forceActiveFocus();
    const QStringList pages{"Overview",
                            "Devices",
                            "Apple Watch",
                            "AirDrop",
                            "AirPods",
                            "Continuity",
                            "Research Lab / Diagnostics",
                            "About"};
    for (int i = 0; i < pages.size(); ++i) {
      if (i)
        QTest::keyClick(window, Qt::Key_Down);
      QTRY_COMPARE(window->property("currentPage").toInt(), i);
      auto *heading = window->findChild<QObject *>("pageHeading");
      QVERIFY(heading);
      QCOMPARE(heading->property("text").toString(), pages[i]);
      QVERIFY(window->findChild<QObject *>("settingsPage"));
      if (!unavailable && (i == 2 || i == 4)) {
        const auto cap = backend.capability(i == 2 ? "apple_watch_auto_unlock" : "airpods_settings");
        QCOMPARE(cap.value("state").toString(), "unsupported");
      }
    }
    QTest::keyClick(window, Qt::Key_Left, Qt::AltModifier);
    QTRY_COMPARE(window->property("currentPage").toInt(), 6);
    auto *overview = visualItem(nav, "nav0");
    QVERIFY(overview);
    QTest::mouseClick(
        window, Qt::LeftButton, Qt::NoModifier,
        overview
            ->mapToScene(QPointF(overview->width() / 2, overview->height() / 2))
            .toPoint());
    QTRY_COMPARE(window->property("currentPage").toInt(), 0);
    QTest::keyClick(window, Qt::Key_Right, Qt::AltModifier);
    QTRY_COMPARE(window->property("currentPage").toInt(), 1);
    QCOMPARE(nav->property("currentIndex").toInt(), 1);
    auto *inventoryRefresh = window->findChild<QQuickItem *>("refreshKnownDevices");
    QVERIFY(inventoryRefresh);
    auto *inventoryStatus = window->findChild<QObject *>("inventoryStatus");
    QVERIFY(inventoryStatus);
    QCOMPARE(inventory.state(), "idle");
    QCOMPARE(discovery.state(), "idle");
    auto *scan = window->findChild<QQuickItem *>("scanBluetooth");
    QVERIFY(scan);
    qputenv("OC_DISCOVERY_MODE", discoveryOutcome.toUtf8());
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, scan->mapToScene(QPointF(scan->width()/2,scan->height()/2)).toPoint());
    if (!unavailable && discoveryOutcome == "live_update") {
      QTRY_VERIFY(discovery.devices()->count() == 1);
      QCOMPARE(discovery.devices()->displayName("scan-0123456789abcdef-001"), "Watch");
      QTRY_COMPARE(discovery.devices()->displayName("scan-0123456789abcdef-001"), "Renamed watch");
      QVERIFY(discovery.scanning());
      QVERIFY(window->findChild<QQuickItem *>("unverifiedObservationsHeading"));
      QVERIFY(window->findChild<QObject *>("unverifiedObservationsSummary")->property("text").toString().contains("count: 1"));
      QCOMPARE(inventory.devices()->count(), 0);
    }
    const auto expectedDiscovery = unavailable ? QString("cli_unavailable") : discoveryOutcome == "sample" || discoveryOutcome == "empty" || discoveryOutcome == "live_update" ? QString("completed") : discoveryOutcome;
    QTRY_COMPARE(discovery.state(), expectedDiscovery);
    auto *discoveryStatus = window->findChild<QObject *>("discoveryStatus"); QVERIFY(discoveryStatus);
    if (!unavailable && discoveryOutcome == "empty") QVERIFY(discoveryStatus->property("text").toString().contains("No observations"));
    if (!unavailable && discoveryOutcome == "sample") QCOMPARE(discovery.devices()->count(),1);
    if (!unavailable && discoveryOutcome == "live_update") QCOMPARE(discovery.devices()->count(),1);
    if (!unavailable && discoveryOutcome == "cleanup_incomplete") QVERIFY(window->findChild<QQuickItem *>("discoveryCleanup")->isVisible());
    if (!unavailable) {
      QTest::qWait(50); // Allow Controls/layout bindings to settle before another click.
      qputenv("OC_DISCOVERY_MODE", "cancel");
      QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, scan->mapToScene(QPointF(scan->width()/2,scan->height()/2)).toPoint());
      QTRY_VERIFY(discovery.scanning()); QTest::qWait(100);
      auto *cancel = window->findChild<QQuickItem *>("cancelBluetoothScan"); QVERIFY(cancel); QVERIFY(cancel->isVisible());
      QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, cancel->mapToScene(QPointF(cancel->width()/2,cancel->height()/2)).toPoint());
      QTRY_COMPARE(discovery.state(), "cancelled"); QCOMPARE(discovery.cleanupStatus(), "complete");
      QTest::qWait(50);
      // Leaving Devices requests cleanup without restarting discovery on return.
      QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier, scan->mapToScene(QPointF(scan->width()/2,scan->height()/2)).toPoint());
      QTRY_VERIFY(discovery.scanning()); QTest::qWait(100);
      window->setProperty("currentPage",2); QTRY_COMPARE(discovery.state(),"cancelled");
      window->setProperty("currentPage",1); QTest::qWait(50); QVERIFY(!discovery.scanning());
      inventoryRefresh = window->findChild<QQuickItem *>("refreshKnownDevices"); QVERIFY(inventoryRefresh);
      inventoryStatus = window->findChild<QObject *>("inventoryStatus"); QVERIFY(inventoryStatus);
    }
    // Scroll the actual Flickable to the independent inventory controls.
    auto *page = window->findChild<QQuickItem *>("settingsPage");
    auto *flick = qvariant_cast<QObject *>(page->property("contentItem")); QVERIFY(flick);
    const auto y = inventoryRefresh->mapToScene(QPointF()).y() - page->mapToScene(QPointF()).y();
    flick->setProperty("contentY", y - 30); QTest::qWait(50);
    qputenv("OC_INVENTORY_MODE", inventoryOutcome.toUtf8());
    QTest::mouseClick(window, Qt::LeftButton, Qt::NoModifier,
                      inventoryRefresh->mapToScene(QPointF(inventoryRefresh->width() / 2, inventoryRefresh->height() / 2)).toPoint());
    const QString expectedInventoryState = unavailable ? "cli_unavailable" : inventoryOutcome == "bluez_unavailable" ? "bluez_unavailable" : "loaded";
    QTRY_COMPARE(inventory.state(), expectedInventoryState);
    if (!unavailable && inventoryOutcome == "bluez_unavailable") {
      QVERIFY(inventoryStatus->property("text").toString().contains("BlueZ is unavailable"));
      QCOMPARE(inventory.devices()->count(), 0);
    } else if (!unavailable) {
      const bool emptyInventory = inventoryOutcome == "empty";
      QCOMPARE(inventory.devices()->count(), emptyInventory ? 0 : 1);
      QVERIFY(inventoryStatus->property("text").toString().contains("snapshot"));
      QVERIFY(window->findChild<QObject *>("inventorySummary")->property("text").toString().contains(emptyInventory ? "0" : "1"));
      if (!emptyInventory) {
        QVERIFY(visualItem(window->findChild<QQuickItem *>("settingsPage"), "knownDeviceCardDetail"));
        auto *detail = visualItem(window->findChild<QQuickItem *>("settingsPage"), "knownDeviceCardDetail");
        QVERIFY(detail->property("text").toString().contains("BlueZ reports connected: no"));
      }
    }
    auto *refresh = window->findChild<QQuickItem *>("refresh");
    QVERIFY(refresh);
    QTest::mouseClick(
        window, Qt::LeftButton, Qt::NoModifier,
        refresh
            ->mapToScene(QPointF(refresh->width() / 2, refresh->height() / 2))
            .toPoint());
    QTRY_COMPARE(backend.reportState(),
                 unavailable ? "cli_unavailable" : "loaded");
    for (const auto &size :
         {QSize(1280, 720), QSize(2560, 1440), QSize(760, 560)}) {
      window->resize(size);
      QTest::qWait(50);
      auto *page = window->findChild<QQuickItem *>("settingsPage");
      QVERIFY(page);
      QVERIFY(page->width() > 400);
      QVERIFY(page->height() > 100);
    }
    window->resize(1280, 720);
    window->setProperty("currentPage", 0);
    QTest::qWait(100);
    const auto output = qEnvironmentVariable("OC_UI_SCREENSHOT");
    const auto rendered = window->grabWindow();
    QVERIFY(!rendered.isNull());
    QCOMPARE(rendered.pixelColor(1200, 700).lightness() < 128, dark);
    if (!output.isEmpty() && !unavailable)
      QVERIFY(window->grabWindow().save(dark ? output + ".dark.png" : output));
    QCOMPARE(warnings.count(), 0);
    window->close();
  }
};
int main(int argc, char **argv) {
  QGuiApplication app(argc, argv);
  QQuickStyle::setStyle("Fusion");
  UiTest test;
  return QTest::qExec(&test, argc, argv);
}
#include "tst_ui.moc"
