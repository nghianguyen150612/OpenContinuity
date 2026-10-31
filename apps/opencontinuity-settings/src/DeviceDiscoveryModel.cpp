#include "DeviceDiscoveryModel.h"
QVariant DeviceDiscoveryModel::data(const QModelIndex &index, int role) const {
  if (!index.isValid() || index.row() < 0 || index.row() >= m_rows.size()) return {};
  const auto &row = m_rows[index.row()];
  switch (role) {
  case IdRole: return row.id;
  case DisplayNameRole: return row.displayName;
  case StaleRole: return m_stale;
  default: return {};
  }
}
QHash<int, QByteArray> DeviceDiscoveryModel::roleNames() const {
  return {{IdRole, "deviceId"}, {DisplayNameRole, "displayName"}, {StaleRole, "stale"}};
}
void DeviceDiscoveryModel::replace(QList<DiscoveryRow> rows) {
  beginResetModel(); m_rows = std::move(rows); m_stale = false; endResetModel(); emit changed();
}
void DeviceDiscoveryModel::clear() {
  beginResetModel(); m_rows.clear(); m_stale = false; endResetModel(); emit changed();
}
bool DeviceDiscoveryModel::contains(const QString &id) const {
  for (const auto &row : m_rows) if (row.id == id) return true;
  return false;
}
QString DeviceDiscoveryModel::displayName(const QString &id) const {
  for (const auto &row : m_rows) if (row.id == id) return row.displayName;
  return {};
}
void DeviceDiscoveryModel::upsert(DiscoveryRow row) {
  for (qsizetype i = 0; i < m_rows.size(); ++i) {
    if (m_rows[i].id == row.id) {
      if (m_rows[i].displayName == row.displayName) return;
      m_rows[i].displayName = std::move(row.displayName);
      emit dataChanged(index(i, 0), index(i, 0), {DisplayNameRole});
      emit changed();
      return;
    }
  }
  const auto rowIndex = m_rows.size();
  beginInsertRows({}, rowIndex, rowIndex); m_rows.append(std::move(row)); endInsertRows(); emit changed();
}
void DeviceDiscoveryModel::markStale() {
  if (m_stale) return;
  m_stale = true;
  if (!m_rows.isEmpty()) emit dataChanged(index(0, 0), index(m_rows.size() - 1, 0), {StaleRole});
  emit changed();
}
