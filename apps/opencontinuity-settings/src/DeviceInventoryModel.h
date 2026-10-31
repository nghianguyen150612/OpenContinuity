#pragma once
#include <QAbstractListModel>
#include <QVariantList>

struct DeviceInventoryRow {
  QString id, adapterId, displayName;
  QVariant connected, paired, trusted;
  bool stale = false;
};

class DeviceInventoryModel final : public QAbstractListModel {
  Q_OBJECT
  Q_PROPERTY(int count READ count NOTIFY countChanged)
  Q_PROPERTY(bool stale READ stale NOTIFY staleChanged)
public:
  enum Roles { IdRole = Qt::UserRole + 1, AdapterIdRole, DisplayNameRole,
               ConnectedRole, PairedRole, TrustedRole, StaleRole };
  explicit DeviceInventoryModel(QObject *parent = nullptr);
  int rowCount(const QModelIndex &parent = {}) const override;
  QVariant data(const QModelIndex &index, int role) const override;
  QHash<int, QByteArray> roleNames() const override;
  int count() const { return m_rows.size(); }
  bool stale() const { return m_stale; }
  void replace(QList<DeviceInventoryRow> rows, bool stale = false);
  void markStale();
signals:
  void countChanged();
  void staleChanged();
private:
  QList<DeviceInventoryRow> m_rows;
  bool m_stale = false;
};
