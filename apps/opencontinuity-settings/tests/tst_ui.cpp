#include "DoctorProvider.h"
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
    QTest::newRow("loaded-light") << false << false;
    QTest::newRow("loaded-dark") << false << true;
    QTest::newRow("missing-cli") << true << false;
  }
  void realWindowAndNavigation() {
    QFETCH(bool, unavailable);
    QFETCH(bool, dark);
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
    qmlRegisterUncreatableType<StatusModel>("OpenContinuity.Backend", 1, 0,
                                            "StatusModel", "Owned");
    qmlRegisterUncreatableType<DoctorProvider>("OpenContinuity.Backend", 1, 0,
                                               "DoctorProvider", "Owned");
    QQmlApplicationEngine engine;
    QSignalSpy warnings(&engine, &QQmlEngine::warnings);
    engine.rootContext()->setContextProperty("doctor", &backend);
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
