#include "StatusModel.h"
QVariant StatusModel::data(const QModelIndex &index, int role) const {
  if (!index.isValid() || index.row() < 0 || index.row() >= m_rows.size())
    return {};
  const auto &r = m_rows[index.row()];
  switch (role) {
  case Id:
    return r.id;
  case Title:
    return r.title;
  case State:
    return r.state;
  case Summary:
    return r.summary;
  default:
    return {};
  }
}
void StatusModel::replace(QVector<StatusRow> rows) {
  beginResetModel();
  m_rows = std::move(rows);
  endResetModel();
}
QVariantMap StatusModel::capability(const QString &id) const {
  for (const auto &r : m_rows)
    if (r.id == id)
      return {{"state", r.state}, {"summary", r.summary}};
  return {{"state", "unavailable"},
          {"summary", "Load a valid CLI report to view this capability."}};
}
