#include "DoctorProvider.h"
#include <QCoreApplication>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTemporaryFile>
#include <QTest>

static QString fixtureExecutable;
static QByteArray validReport() {
  QFile f(":/report.json");
  if (!f.open(QIODevice::ReadOnly))
    return {};
  return f.readAll();
}
class BackendTest : public QObject {
  Q_OBJECT
private slots:
  void init() { qunsetenv("OC_SETTINGS_TEST_MODE"); }
  void validAndModels() {
    const auto report = DoctorReport::parse(validReport());
    QVERIFY(report);
    QCOMPARE(report->capabilities.size(), 3);
    QCOMPARE(report->trust.size(), 6);
    QCOMPARE(report->capabilities[0].state, "unsupported");
    QCOMPARE(report->trust[0].state, "unknown");
    StatusModel model;
    QSignalSpy reset(&model, &QAbstractItemModel::modelReset);
    model.replace(report->capabilities);
    QCOMPARE(reset.count(), 1);
    QCOMPARE(model.rowCount(), 3);
    QCOMPARE(model.data(model.index(0), StatusModel::Id).toString(),
             "apple_watch_auto_unlock");
    QCOMPARE(model.data(model.index(0), StatusModel::State).toString(),
             "unsupported");
    QVERIFY(!model.data(QModelIndex(), StatusModel::State).isValid());
    QCOMPARE(model.capability("missing")["state"].toString(), "unavailable");
  }
  void allEnums() {
    for (const auto &state :
         {"supported", "experimental", "unsupported", "blocked", "unknown"}) {
      auto o = QJsonDocument::fromJson(validReport()).object();
      auto rows = o["capabilities"].toArray();
      auto r = rows[0].toObject();
      r["state"] = state;
      rows[0] = r;
      o["capabilities"] = rows;
      const auto report = DoctorReport::parse(QJsonDocument(o).toJson());
      QVERIFY(report);
      QCOMPARE(report->capabilities[0].state, state);
    }
  }
  void invalid_data() {
    QTest::addColumn<QByteArray>("bytes");
    QTest::newRow("malformed") << QByteArray("{bad}");
    QTest::newRow("truncated") << validReport().left(100);
    QTest::newRow("empty") << QByteArray();
    QTest::newRow("oversized") << QByteArray(DoctorReport::MaxOutput + 1, ' ');
    QTest::newRow("array") << QByteArray("[]");
    QTest::newRow("trailing") << (validReport() + "garbage");
    QTest::newRow("duplicate-root-member")
        << QByteArray(validReport())
               .replace("\"schema_version\":1",
                        "\"schema_version\":2,\"schema_version\":1");
    const auto original = QJsonDocument::fromJson(validReport()).object();
    auto add = [](const char *name, const QJsonObject &o) {
      QTest::newRow(name) << QJsonDocument(o).toJson();
    };
    for (const auto &key : original.keys()) {
      auto o = original;
      o.remove(key);
      add(qPrintable("missing-" + key), o);
    }
    for (const auto &version : {QJsonValue(2), QJsonValue("1"), QJsonValue(1.5),
                                QJsonValue(true), QJsonValue()}) {
      auto o = original;
      o["schema_version"] = version;
      add(qPrintable("schema-" + QJsonDocument(QJsonArray{version})
                                     .toJson(QJsonDocument::Compact)),
          o);
    }
    auto o = original;
    o["extra"] = true;
    add("unexpected-root", o);
    o = original;
    o["project"] = "Other";
    add("project", o);
    o = original;
    o["version"] = QString(65, 'a');
    add("long-version", o);
    o = original;
    o["capabilities"] = QJsonObject();
    add("capabilities-type", o);
    auto rows = original["capabilities"].toArray();
    rows[1] = rows[0];
    o = original;
    o["capabilities"] = rows;
    add("duplicate-id", o);
    rows = original["capabilities"].toArray();
    rows.removeLast();
    o = original;
    o["capabilities"] = rows;
    add("missing-capability", o);
    for (const auto &key : {"id", "state", "summary", "extra"}) {
      rows = original["capabilities"].toArray();
      auto r = rows[0].toObject();
      if (QString(key) == "id")
        r[key] = "future_id";
      else if (QString(key) == "state")
        r[key] = "ready";
      else if (QString(key) == "summary")
        r[key] = QString(513, 'a');
      else
        r[key] = true;
      rows[0] = r;
      o = original;
      o["capabilities"] = rows;
      add(qPrintable("capability-" + QString(key)), o);
    }
    rows = original["capabilities"].toArray();
    auto r = rows[0].toObject();
    r["summary"] = "unsafe\ncontrol";
    rows[0] = r;
    o = original;
    o["capabilities"] = rows;
    add("control-text", o);
    for (const auto &value :
         {QJsonValue("supported"), QJsonValue(true), QJsonValue()}) {
      o = original;
      o["watch_auto_unlock_trust"] = value;
      add(qPrintable("trust-type-" + QString::number(value.type())), o);
    }
    const auto trust = original["watch_auto_unlock_trust"].toObject();
    for (const auto &key : trust.keys()) {
      auto t = trust;
      t.remove(key);
      o = original;
      o["watch_auto_unlock_trust"] = t;
      add(qPrintable("trust-missing-" + key), o);
      t = trust;
      t[key] = "ready";
      o["watch_auto_unlock_trust"] = t;
      add(qPrintable("trust-enum-" + key), o);
    }
    auto t = trust;
    t["authorization"] = "supported";
    o = original;
    o["watch_auto_unlock_trust"] = t;
    add("trust-extra", o);
  }
  void invalid() {
    QFETCH(QByteArray, bytes);
    QVERIFY(!DoctorReport::parse(bytes));
  }
  void processSuccess() {
    DoctorProvider p({fixtureExecutable});
    QSignalSpy changes(&p, &DoctorProvider::changed);
    p.refresh();
    QVERIFY(p.loading());
    QCOMPARE(p.reportState(), "loading");
    QTRY_COMPARE(p.reportState(), "loaded");
    QVERIFY(!p.loading());
    QCOMPARE(p.schemaVersion(), 1);
    QCOMPARE(p.capabilities()->rowCount(), 3);
    QCOMPARE(p.trustStages()->rowCount(), 6);
    QCOMPARE(p.reportVersion(), "0.1.0");
    QVERIFY(!p.lastUpdated().isEmpty());
    QVERIFY(changes.count() >= 2);
  }
  void failure_data() {
    QTest::addColumn<QByteArray>("mode");
    QTest::addColumn<QString>("expected");
    QTest::newRow("timeout") << QByteArray("timeout") << QString("timed_out");
    QTest::newRow("nonzero")
        << QByteArray("nonzero") << QString("process_error");
    QTest::newRow("crash") << QByteArray("crash") << QString("process_error");
    for (const auto *mode : {"invalid", "oversized", "stderr-large", "stderr"})
      QTest::newRow(mode) << QByteArray(mode) << QString("invalid_report");
  }
  void failure() {
    QFETCH(QByteArray, mode);
    QFETCH(QString, expected);
    qputenv("OC_SETTINGS_TEST_MODE", mode);
    DoctorProvider p({fixtureExecutable, 100});
    p.refresh();
    QTRY_COMPARE(p.reportState(), expected);
    QVERIFY(!p.loading());
    QCOMPARE(p.schemaVersion(), 0);
    QCOMPARE(p.capabilities()->rowCount(), 0);
    QVERIFY(!p.lastError().contains("untrusted"));
  }
  void missingExecutable() {
    for (const auto &path : {QString("/no/such/opencontinuity"),
                             QString("relative-command;bad")}) {
      DoctorProvider p({path});
      p.refresh();
      QCOMPARE(p.reportState(), "cli_unavailable");
      QVERIFY(!p.loading());
    }
    QTemporaryFile file;
    QVERIFY(file.open());
    DoctorProvider p({file.fileName()});
    p.refresh();
    QCOMPARE(p.reportState(), "cli_unavailable");
  }
  void failedToStart() {
    QTemporaryFile file;
    QVERIFY(file.open());
    file.write("invalid executable format");
    file.close();
    QVERIFY(
        file.setPermissions(QFileDevice::ReadOwner | QFileDevice::ExeOwner));
    DoctorProvider p({file.fileName()});
    p.refresh();
    QTRY_COMPARE(p.reportState(), "cli_unavailable");
  }
  void concurrentRefresh() {
    qputenv("OC_SETTINGS_TEST_MODE", "delay");
    DoctorProvider p({fixtureExecutable});
    p.refresh();
    QSignalSpy changes(&p, &DoctorProvider::changed);
    p.refresh();
    p.refresh();
    QCOMPARE(changes.count(), 0);
    QVERIFY(p.loading());
    QTRY_COMPARE(p.reportState(), "loaded");
    QCOMPARE(p.capabilities()->rowCount(), 3);
  }
  void cancellationAndStaleIsolation() {
    qputenv("OC_SETTINGS_TEST_MODE", "delay");
    DoctorProvider p({fixtureExecutable});
    p.refresh();
    p.cancel();
    QCOMPARE(p.reportState(), "cancelled");
    QVERIFY(!p.loading());
    qputenv("OC_SETTINGS_TEST_MODE", "invalid");
    p.refresh();
    QTRY_COMPARE(p.reportState(), "invalid_report");
    QTest::qWait(300);
    QCOMPARE(p.reportState(), "invalid_report");
    QCOMPARE(p.schemaVersion(), 0);
    qunsetenv("OC_SETTINGS_TEST_MODE");
    p.refresh();
    QTRY_COMPARE(p.reportState(), "loaded");
    qputenv("OC_SETTINGS_TEST_MODE", "nonzero");
    p.refresh();
    QTRY_COMPARE(p.reportState(), "stale");
    QCOMPARE(p.failureState(), "process_error");
    QCOMPARE(p.capabilities()->rowCount(), 3);
    qunsetenv("OC_SETTINGS_TEST_MODE");
    p.refresh();
    QTRY_COMPARE(p.reportState(), "loaded");
    QVERIFY(p.lastError().isEmpty());
  }
  void ageStale() {
    DoctorProvider p({fixtureExecutable, 3000, 100});
    p.refresh();
    QTRY_COMPARE(p.reportState(), "loaded");
    QTRY_COMPARE(p.reportState(), "stale");
    QCOMPARE(p.schemaVersion(), 1);
  }
  void realRustContract() {
    const auto cli = qEnvironmentVariable("OC_REAL_CLI");
    if (cli.isEmpty())
      QSKIP("Set OC_REAL_CLI to test the built Rust binary");
    DoctorProvider p({cli});
    p.refresh();
    QTRY_COMPARE(p.reportState(), "loaded");
    QCOMPARE(p.capability("airdrop")["state"].toString(), "unsupported");
    QCOMPARE(p.trustStages()
                 ->data(p.trustStages()->index(0), StatusModel::State)
                 .toString(),
             "unknown");
  }
};
int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  if (argc != 2)
    return 2;
  fixtureExecutable = QString::fromLocal8Bit(argv[1]);
  BackendTest test;
  return QTest::qExec(&test, 1, argv);
}
#include "tst_backend.moc"
