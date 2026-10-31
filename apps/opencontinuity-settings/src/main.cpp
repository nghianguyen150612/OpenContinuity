#include "DoctorProvider.h"
#include "DeviceInventoryProvider.h"
#include "DeviceDiscoveryProvider.h"
#include <QCommandLineParser>
#include <QDir>
#include <QGuiApplication>
#include <QIcon>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>

int main(int argc, char **argv) {
  QGuiApplication app(argc, argv);
  QCoreApplication::setApplicationName("opencontinuity-settings");
  QCoreApplication::setApplicationVersion("0.1.0");
  QGuiApplication::setDesktopFileName("opencontinuity-settings");
  QGuiApplication::setWindowIcon(QIcon(":/assets/opencontinuity-settings.svg"));
  QQuickStyle::setStyle("Fusion");
  QCommandLineParser parser;
  parser.setApplicationDescription(
      "OpenContinuity capabilities, known devices and opt-in Bluetooth LE discovery");
  parser.addHelpOption();
  parser.addVersionOption();
  parser.addOption({"cli",
                    "Trusted absolute path to the OpenContinuity CLI (fixed "
                    "CLI arguments).",
                    "path"});
  parser.process(app);
  const QString executable = parser.isSet("cli")
                                 ? parser.value("cli")
                                 : QDir(QCoreApplication::applicationDirPath())
                                       .filePath("opencontinuity");
  DoctorProvider backend({executable});
  DeviceInventoryProvider inventory({executable});
  DeviceDiscoveryProvider discovery({executable});
  qmlRegisterUncreatableType<StatusModel>(
      "OpenContinuity.Backend", 1, 0, "StatusModel", "Owned by DoctorProvider");
  qmlRegisterUncreatableType<DoctorProvider>("OpenContinuity.Backend", 1, 0,
                                             "DoctorProvider",
                                             "Owned by the application");
  qmlRegisterUncreatableType<DeviceInventoryModel>("OpenContinuity.Backend", 1, 0,
                                                   "DeviceInventoryModel", "Owned by DeviceInventoryProvider");
  qmlRegisterUncreatableType<DeviceInventoryProvider>("OpenContinuity.Backend", 1, 0,
                                                      "DeviceInventoryProvider", "Owned by the application");
  qmlRegisterUncreatableType<DeviceDiscoveryModel>("OpenContinuity.Backend", 1, 0, "DeviceDiscoveryModel", "Owned by DeviceDiscoveryProvider");
  qmlRegisterUncreatableType<DeviceDiscoveryProvider>("OpenContinuity.Backend", 1, 0, "DeviceDiscoveryProvider", "Owned by the application");
  QQmlApplicationEngine engine;
  engine.rootContext()->setContextProperty("doctor", &backend);
  engine.rootContext()->setContextProperty("deviceInventory", &inventory);
  engine.rootContext()->setContextProperty("deviceDiscovery", &discovery);
  engine.load(QUrl("qrc:/qml/Main.qml"));
  if (engine.rootObjects().isEmpty())
    return 1;
  QTimer::singleShot(0, &backend, &DoctorProvider::refresh);
  QObject::connect(&app, &QCoreApplication::aboutToQuit, &backend,
                   &DoctorProvider::cancel);
  QObject::connect(&app, &QCoreApplication::aboutToQuit, &inventory,
                   &DeviceInventoryProvider::cancel);
  QObject::connect(&app, &QCoreApplication::aboutToQuit, &discovery, &DeviceDiscoveryProvider::shutdown);
  return app.exec();
}
