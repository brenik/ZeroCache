# Changelog


## [0.1.1] - 2025-11-09

### Added
- **Typed field support** - Added type suffixes for numeric fields (`:u64`, `:i64`, `:f64`)
  - Example: `X-Upsert-Field: objectID,name,price:f64,quantity:u64`
  - Enables proper numeric indexing and range queries
- **Native range queries** - Tantivy-based range filtering for numeric fields
  - `?filter_min_price=100&filter_max_price=500` now uses index instead of post-filtering
  - Significantly faster performance for numeric range queries
- **Field type display** - `/trees` endpoint now shows field types
  - Example: `price_default(f64)`, `name(text)`, `objectID(primary, text)`

### Changed
- **Query limit enforcement** - `max_scan_limit` now properly caps all queries
  - Prevents excessive resource usage from large limit values
  - `?limit=2000` with `max_scan_limit=1000` now returns max 1000 items

### Fixed
- **Quote handling in queries** - Removed automatic quote wrapping in field filters
  - Users control quote usage: `?categoryIds=52` (token) vs `?categoryIds="52"` (exact)
  - Fixes search issues with array fields like `categoryIds`
- **Numeric field indexing** - Fixed TEXT-based indexing for numeric fields
  - Numeric fields now properly indexed as U64/I64/F64 types
  - Range queries now work correctly on price and quantity fields

### Performance
- Range queries now use Tantivy index (10-100x faster than post-filtering)

## [0.1.0] - 2024-11-01

### Added
- Initial release
- Sled embedded key-value store
- Tantivy full-text search integration
- REST API with CRUD operations
- IP-based access control
- Rate limiting for public endpoints
- NDJSON bulk import support
- Dynamic schema with flexible indexing
- Integration test suite (14 tests)
