#pragma once
#include <QByteArray>
#include <QString>
#include <QVector>
#include <optional>

struct StatusRow {
  QString id, title, state, summary;
};
struct DoctorReport {
  QString version;
  QVector<StatusRow> capabilities, trust;
  static constexpr qsizetype MaxOutput = 16384;
  static std::optional<DoctorReport> parse(const QByteArray &bytes);
};
