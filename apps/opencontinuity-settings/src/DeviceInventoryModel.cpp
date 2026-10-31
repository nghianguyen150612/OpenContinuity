#include "DeviceInventoryModel.h"

DeviceInventoryModel::DeviceInventoryModel(QObject *parent)
    : QAbstractListModel(parent) {}
int DeviceInventoryModel::rowCount(const QModelIndex &parent) const {
  return parent.isValid() ? 0 : m_rows.size();
}
QVariant DeviceInventoryModel::data(const QModelIndex &index, int role) const {
  if (!index.isValid() || index.row() < 0 || index.row() >= m_rows.size())
    return {};
  const auto &row = m_rows[index.row()];
  switch (role) {
  case IdRole: return row.id;
  case AdapterIdRole: return row.adapterId;
  case DisplayNameRole: return row.displayName;
  case ConnectedRole: return row.connected;
  case PairedRole: return row.paired;
  case TrustedRole: return row.trusted;
  case StaleRole: return row.stale;
  default: return {};
  }
}
QHash<int, QByteArray> DeviceInventoryModel::roleNames() const {
  return {{IdRole, "deviceId"}, {AdapterIdRole, "adapterId"},
          {DisplayNameRole, "displayName"}, {ConnectedRole, "connected"},
          {PairedRole, "paired"}, {TrustedRole, "trusted"},
          {StaleRole, "stale"}};
}
void DeviceInventoryModel::replace(QList<DeviceInventoryRow> rows,
                                   bool isStale) {
  const auto oldCount = m_rows.size();
  const auto oldStale = m_stale;
  beginResetModel();
  m_rows = std::move(rows);
  m_stale = isStale;
  endResetModel();
  if (oldCount != m_rows.size()) emit countChanged();
  if (oldStale != m_stale) emit staleChanged();
}
void DeviceInventoryModel::markStale() {
  if (m_stale) return;
  m_stale = true;
  for (auto &row : m_rows) row.stale = true;
  if (!m_rows.isEmpty()) emit dataChanged(index(0, 0), index(m_rows.size() - 1, 0), {StaleRole});
  emit staleChanged();
}
