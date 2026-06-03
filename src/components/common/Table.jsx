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
  caption,
  className = '',
}) {
  const handleHeaderClick = (column) => {
    if (column.sortable && onSort) {
      onSort(column.key);
    }
  };

  const handleHeaderKeyDown = (e, column) => {
    if (column.sortable && onSort && (e.key === 'Enter' || e.key === ' ')) {
      e.preventDefault();
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

  const getAriaSortValue = (column) => {
    if (!column.sortable) return undefined;
    if (sortKey !== column.key) return 'none';
    return sortOrder === 'asc' ? 'ascending' : 'descending';
  };

  const renderSortIcon = (column) => {
    if (!column.sortable) return null;
    if (sortKey !== column.key) {
      return <span className="sort-icon sort-icon-neutral" aria-hidden="true">↕</span>;
    }
    return (
      <span className={`sort-icon sort-icon-${sortOrder === 'asc' ? 'active' : 'active-desc'}`} aria-hidden="true">
        {sortOrder === 'asc' ? '↑' : '↓'}
      </span>
    );
  };

  return (
    <div className={`table-wrapper ${className}`}>
      <div className="table-container" role="region" aria-label={caption}>
        <table className="table">
          {caption && <caption className="table-caption">{caption}</caption>}
          <thead className="table-head">
            <tr className="table-row table-row-head">
              {selectable && (
                <th className="table-header table-cell-checkbox" scope="col">
                  <label className="checkbox-wrapper">
                    <input
                      type="checkbox"
                      checked={isAllSelected}
                      ref={(el) => { if (el) el.indeterminate = isSomeSelected; }}
                      onChange={handleSelectAll}
                      className="checkbox-input"
                      aria-label={isAllSelected ? 'Deselect all rows' : 'Select all rows'}
                    />
                    <span className="checkbox-custom" aria-hidden="true" />
                  </label>
                </th>
              )}
              {columns.map((column) => (
                <th
                  key={column.key}
                  scope="col"
                  className={`table-header ${column.sortable ? 'sortable' : ''} ${sortKey === column.key ? 'sorted' : ''}`}
                  style={{ width: column.width }}
                  aria-sort={getAriaSortValue(column)}
                  onClick={() => handleHeaderClick(column)}
                  onKeyDown={(e) => handleHeaderKeyDown(e, column)}
                  tabIndex={column.sortable ? 0 : undefined}
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
                    aria-selected={selectable ? isSelected : undefined}
                  >
                    {selectable && (
                      <td className="table-cell table-cell-checkbox">
                        <label className="checkbox-wrapper">
                          <input
                            type="checkbox"
                            checked={isSelected}
                            onChange={() => handleRowSelect(rowIndex)}
                            className="checkbox-input"
                            aria-label={`Select row ${rowIndex + 1}`}
                          />
                          <span className="checkbox-custom" aria-hidden="true" />
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
