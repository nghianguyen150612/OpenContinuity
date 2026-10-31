#pragma once
#include <QAbstractListModel>

struct DiscoveryRow { QString id, displayName; };
class DeviceDiscoveryModel final : public QAbstractListModel {
  Q_OBJECT
  Q_PROPERTY(int count READ count NOTIFY changed)
  Q_PROPERTY(bool stale READ stale NOTIFY changed)
public:
  enum Roles { IdRole = Qt::UserRole + 1, DisplayNameRole, StaleRole };
  explicit DeviceDiscoveryModel(QObject *parent = nullptr) : QAbstractListModel(parent) {}
  int rowCount(const QModelIndex &parent = {}) const override { return parent.isValid() ? 0 : m_rows.size(); }
  QVariant data(const QModelIndex &index, int role) const override;
  QHash<int, QByteArray> roleNames() const override;
  int count() const { return m_rows.size(); }
  bool stale() const { return m_stale; }
  void replace(QList<DiscoveryRow> rows);
  void clear();
  bool contains(const QString &id) const;
  QString displayName(const QString &id) const;
  void upsert(DiscoveryRow row);
  void markStale();
signals:
  void changed();
private:
  QList<DiscoveryRow> m_rows;
  bool m_stale = false;
};
