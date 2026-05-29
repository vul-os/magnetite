import Pagination from '../Pagination';
import EmptyState from '../empty/EmptyState';
import './Table.css';

export default function Table({
  columns,
  data,
  onSort,
  sortKey,
  sortOrder,
  selectable = false,
  selectedRows = [],
  onRowSelect,
  emptyIcon,
  emptyTitle = 'No data found',
  emptyDescription = 'There are no items to display.',
  pagination,
  className = '',
}) {
  const handleHeaderClick = (column) => {
    if (column.sortable && onSort) {
      onSort(column.key);
    }
  };

  const handleSelectAll = () => {
    if (selectedRows.length === data.length) {
      onRowSelect?.([]);
    } else {
      onRowSelect?.(data.map((_, index) => index));
    }
  };

  const handleRowSelect = (index) => {
    const newSelected = selectedRows.includes(index)
      ? selectedRows.filter(i => i !== index)
      : [...selectedRows, index];
    onRowSelect?.(newSelected);
  };

  const isAllSelected = data.length > 0 && selectedRows.length === data.length;
  const isSomeSelected = selectedRows.length > 0 && selectedRows.length < data.length;

  const renderSortIcon = (column) => {
    if (!column.sortable) return null;
    if (sortKey !== column.key) {
      return <span className="sort-icon sort-icon-neutral">↕</span>;
    }
    return (
      <span className={`sort-icon sort-icon-${sortOrder === 'asc' ? 'active' : 'active-desc'}`}>
        {sortOrder === 'asc' ? '↑' : '↓'}
      </span>
    );
  };

  return (
    <div className={`table-wrapper ${className}`}>
      <div className="table-container">
        <table className="table">
          <thead className="table-head">
            <tr className="table-row table-row-head">
              {selectable && (
                <th className="table-header table-cell-checkbox">
                  <label className="checkbox-wrapper">
                    <input
                      type="checkbox"
                      checked={isAllSelected}
                      ref={(el) => { if (el) el.indeterminate = isSomeSelected; }}
                      onChange={handleSelectAll}
                      className="checkbox-input"
                    />
                    <span className="checkbox-custom" />
                  </label>
                </th>
              )}
              {columns.map((column) => (
                <th
                  key={column.key}
                  className={`table-header ${column.sortable ? 'sortable' : ''} ${sortKey === column.key ? 'sorted' : ''}`}
                  style={{ width: column.width }}
                  onClick={() => handleHeaderClick(column)}
                >
                  <div className="header-content">
                    <span className="header-label">{column.label}</span>
                    {renderSortIcon(column)}
                  </div>
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="table-body">
            {data.length === 0 ? (
              <tr className="table-row table-row-empty">
                <td colSpan={columns.length + (selectable ? 1 : 0)} className="table-cell table-cell-empty">
                  <EmptyState
                    icon={emptyIcon}
                    title={emptyTitle}
                    description={emptyDescription}
                  />
                </td>
              </tr>
            ) : (
              data.map((row, rowIndex) => {
                const isSelected = selectedRows.includes(rowIndex);
                return (
                  <tr
                    key={rowIndex}
                    className={`table-row ${isSelected ? 'selected' : ''}`}
                  >
                    {selectable && (
                      <td className="table-cell table-cell-checkbox">
                        <label className="checkbox-wrapper">
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={() => handleRowSelect(rowIndex)}
                            className="checkbox-input"
                          />
                          <span className="checkbox-custom" />
                        </label>
                      </td>
                    )}
                    {columns.map((column) => (
                      <td key={column.key} className="table-cell">
                        {column.render ? column.render(row[column.key], row) : row[column.key]}
                      </td>
                    ))}
                  </tr>
                );
              })
            )}
          </tbody>
        </table>
      </div>
      {pagination && (
        <Pagination
          total={pagination.total || data.length}
          perPage={pagination.perPage || 10}
          currentPage={pagination.currentPage || 1}
          onPageChange={pagination.onPageChange}
          showFirstLast
          className="table-pagination"
        />
      )}
    </div>
  );
}

export function TableHead({ children, className = '' }) {
  return <thead className={`table-head ${className}`}>{children}</thead>;
}

export function TableBody({ children, className = '' }) {
  return <tbody className={`table-body ${className}`}>{children}</tbody>;
}

export function TableRow({ children, className = '', selected = false, onClick }) {
  const classes = [
    'table-row',
    selected ? 'selected' : '',
    onClick ? 'clickable' : '',
    className,
  ].filter(Boolean).join(' ');
  return (
    <tr className={classes} onClick={onClick}>
      {children}
    </tr>
  );
}

export function TableHeader({ children, className = '', sortable = false, sorted = false, onClick }) {
  const classes = [
    'table-header',
    sortable ? 'sortable' : '',
    sorted ? 'sorted' : '',
    className,
  ].filter(Boolean).join(' ');
  return (
    <th className={classes} onClick={onClick}>
      {children}
    </th>
  );
}

export function TableCell({ children, className = '' }) {
  return <td className={`table-cell ${className}`}>{children}</td>;
}
