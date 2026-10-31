#pragma once
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QSet>
namespace JsonValidation {
inline bool uniqueMembers(const QByteArray &bytes) {
  QVector<QSet<QString>> objects;
  for (qsizetype i = 0; i < bytes.size(); ++i) {
    if (bytes[i] == '{') { objects.append(QSet<QString>{}); if (objects.size() > 4) return false; }
    else if (bytes[i] == '}') { if (objects.isEmpty()) return false; objects.removeLast(); }
    else if (bytes[i] == '"') {
      const auto start = i;
      bool escaped = false;
      for (++i; i < bytes.size(); ++i) {
        if (escaped) { escaped = false; continue; }
        if (bytes[i] == '\\') { escaped = true; continue; }
        if (bytes[i] == '"') break;
      }
      if (i >= bytes.size()) return false;
      auto next = i + 1;
      while (next < bytes.size() && (bytes[next] == ' ' || bytes[next] == '\n' || bytes[next] == '\r' || bytes[next] == '\t')) ++next;
      if (next < bytes.size() && bytes[next] == ':') {
        if (objects.isEmpty()) return false;
        QJsonParseError error;
        const auto decoded = QJsonDocument::fromJson("[" + bytes.mid(start, i - start + 1) + "]", &error);
        if (error.error != QJsonParseError::NoError || !decoded.isArray() || decoded.array().size() != 1 || !decoded.array().at(0).isString()) return false;
        const auto key = decoded.array().at(0).toString();
        if (objects.last().contains(key)) return false;
        objects.last().insert(key);
      }
    }
  }
  return objects.isEmpty();
}
inline bool keys(const QJsonObject &object, const QStringList &expected) {
  const auto actual = object.keys();
  return QSet<QString>(actual.cbegin(), actual.cend()) == QSet<QString>(expected.cbegin(), expected.cend());
}
inline bool safeText(const QJsonValue &value, int maximum) {
  if (!value.isString() || value.toString().isEmpty() || value.toString().size() > maximum) return false;
  for (const auto c : value.toString()) if (c.category() == QChar::Other_Control || c.category() == QChar::Other_Format) return false;
  return true;
}
}
