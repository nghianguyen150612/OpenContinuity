#pragma once
#include "DoctorReport.h"
#include <QAbstractListModel>
class StatusModel : public QAbstractListModel {
  Q_OBJECT
public:
  enum Role { Id = Qt::UserRole + 1, Title, State, Summary };
  explicit StatusModel(QObject *parent = nullptr)
      : QAbstractListModel(parent) {}
  int rowCount(const QModelIndex &parent = {}) const override {
    return parent.isValid() ? 0 : int(m_rows.size());
  }
  QVariant data(const QModelIndex &index, int role) const override;
  QHash<int, QByteArray> roleNames() const override {
    return {{Id, "identifier"},
            {Title, "title"},
            {State, "state"},
            {Summary, "summary"}};
  }
  void replace(QVector<StatusRow> rows);
  QVariantMap capability(const QString &id) const;

private:
  QVector<StatusRow> m_rows;
};
