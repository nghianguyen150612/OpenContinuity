#include "AirdropStatusProvider.h"
#include "AirdropStatusReport.h"
#include <QCoreApplication>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSignalSpy>
#include <QTest>

static QString fixtureExecutable;

static QByteArray validStatus() {
  return R"json({"schemaVersion":1,"kind":"airdrop_backend_status","source":"owly_doctor","authorization":"never_evaluated","physicalQualification":"not_qualified","backend":"blocked","controlPlane":"blocked","dataPlane":"unknown","advertisement":"unknown","receiverAvailable":false,"liveAirdropInteroperability":"unknown","blockReason":"radio_not_prepared","radio":{"state":"available","kernelRelease":"6.12.0-test","driver":"rtw89_8852be","pciId":"10ec:b852","regulatoryDomain":"US","wifiChannel":153,"associated":true,"monitorMode":"available","wirelessInterfaceCount":1,"conflictingWirelessInterfaceCount":0,"awdlInterfaceState":"blocked","awdlLinkLocalAddressPresent":false,"effectiveUidIsRoot":false,"netAdminCapability":false,"netRawCapability":false},"upstreamVersion":"0.4.2"})json";
}

class AirdropStatusTest : public QObject {
  Q_OBJECT
private slots:
  void init() { qunsetenv("OC_AIRDROP_FIXTURE_MODE"); }

  void validReportIsSanitizedAndTyped() {
    const auto report = AirdropStatusReport::parse(validStatus());
    QVERIFY(report);
    QCOMPARE(report->source, "owly_doctor");
    QCOMPARE(report->backend, "blocked");
    QCOMPARE(report->driver, "rtw89_8852be");
    QCOMPARE(report->pciId, "10ec:b852");
    QCOMPARE(report->channel, "153");
    QCOMPARE(report->associated, "yes");
    QVERIFY(!report->receiverAvailable);
    QCOMPARE(report->blockReason, "radio_not_prepared");
  }

  void receiverReadyMustMatchPlanes() {
    auto root = QJsonDocument::fromJson(validStatus()).object();
    auto radio = root.value("radio").toObject();
    root.insert("backend", "ready");
    root.insert("dataPlane", "available");
    root.insert("receiverAvailable", true);
    radio.insert("awdlInterfaceState", "ready");
    radio.insert("awdlLinkLocalAddressPresent", true);
    root.insert("radio", radio);
    const auto report = AirdropStatusReport::parse(QJsonDocument(root).toJson());
    QVERIFY(report);
    QVERIFY(report->receiverAvailable);
    root.insert("receiverAvailable", false);
    QVERIFY(!AirdropStatusReport::parse(QJsonDocument(root).toJson()));
  }

  void invalid_data() {
    QTest::addColumn<QByteArray>("bytes");
    QTest::newRow("empty") << QByteArray();
    QTest::newRow("malformed") << QByteArray("{bad}");
    QTest::newRow("oversized") << QByteArray(64 * 1024 + 1, 'x');
    QTest::newRow("duplicate-key")
        << validStatus().replace("\"schemaVersion\":1",
                                 "\"schemaVersion\":1,\"schemaVersion\":1");
    QTest::newRow("escaped-duplicate-key")
        << validStatus().replace("\"schemaVersion\":1",
                                 "\"schemaVersion\":1,\"schema\\u0056ersion\":1");
    auto root = QJsonDocument::fromJson(validStatus()).object();
    auto changed = root;
    changed.insert("extra", true);
    QTest::newRow("unknown-root-field") << QJsonDocument(changed).toJson();
    changed = root;
    changed.insert("authorization", "allowed");
    QTest::newRow("authorization") << QJsonDocument(changed).toJson();
    changed = root;
    changed.insert("physicalQualification", "pass");
    QTest::newRow("physical-claim") << QJsonDocument(changed).toJson();
    changed = root;
    changed.insert("backend", "simulated_success");
    QTest::newRow("unknown-state") << QJsonDocument(changed).toJson();
    auto radio = root.value("radio").toObject();
    radio.insert("bssid", "00:11:22:33:44:55");
    changed = root;
    changed.insert("radio", radio);
    QTest::newRow("identifier-field") << QJsonDocument(changed).toJson();
    radio = root.value("radio").toObject();
    radio.insert("driver", "driver\nsecret");
    changed = root;
    changed.insert("radio", radio);
    QTest::newRow("unsafe-driver") << QJsonDocument(changed).toJson();
    radio = root.value("radio").toObject();
    radio.insert("pciId", "not-a-pci-id");
    changed = root;
    changed.insert("radio", radio);
    QTest::newRow("unsafe-pci-id") << QJsonDocument(changed).toJson();
  }

  void invalid() {
    QFETCH(QByteArray, bytes);
    QVERIFY(!AirdropStatusReport::parse(bytes));
  }

  void processSuccess() {
    AirdropStatusProvider provider({fixtureExecutable});
    QSignalSpy changes(&provider, &AirdropStatusProvider::changed);
    provider.refresh();
    QCOMPARE(provider.state(), "loading");
    QTRY_COMPARE(provider.state(), "loaded");
    QCOMPARE(provider.report().value("backend").toString(), "blocked");
    QCOMPARE(provider.report().value("authorization").toString(), "never_evaluated");
    QVERIFY(changes.count() >= 2);
  }

  void failureAndDeadline() {
    for (const auto *mode : {"invalid", "stderr", "nonzero"}) {
      qputenv("OC_AIRDROP_FIXTURE_MODE", mode);
      AirdropStatusProvider provider({fixtureExecutable});
      provider.refresh();
      QTRY_VERIFY(provider.state() == "invalid_report" ||
                  provider.state() == "process_error");
      QVERIFY(provider.state() != "loaded");
    }
    qputenv("OC_AIRDROP_FIXTURE_MODE", "delay");
    AirdropStatusProvider timeout({fixtureExecutable, 40});
    timeout.refresh();
    QTRY_COMPARE(timeout.state(), "timed_out");
  }

  void cancellationFencesStaleCallbacks() {
    qputenv("OC_AIRDROP_FIXTURE_MODE", "delay");
    AirdropStatusProvider provider({fixtureExecutable, 2000});
    provider.refresh();
    provider.cancel();
    QCOMPARE(provider.state(), "cancelled");
    qputenv("OC_AIRDROP_FIXTURE_MODE", "invalid");
    provider.refresh();
    QTRY_COMPARE(provider.state(), "invalid_report");
    QTest::qWait(400);
    QCOMPARE(provider.state(), "invalid_report");
  }
};

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  if (argc != 2)
    return 2;
  fixtureExecutable = QString::fromLocal8Bit(argv[1]);
  AirdropStatusTest test;
  return QTest::qExec(&test, 1, argv);
}

#include "tst_airdrop.moc"
