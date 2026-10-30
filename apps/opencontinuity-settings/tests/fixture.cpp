// Controlled native test peer. Never interprets commands, arguments or shell
// text.
#include <QCoreApplication>
#include <QFile>
#include <QThread>
#include <cstdio>
int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  if (app.arguments().mid(1) != QStringList{"doctor", "--json"})
    return 90;
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
  QFile file(":/report.json");
  if (!file.open(QIODevice::ReadOnly))
    return 91;
  const auto bytes = file.readAll();
  std::fwrite(bytes.data(), 1, bytes.size(), stdout);
  return 0;
}
