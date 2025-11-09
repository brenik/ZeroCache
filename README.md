# ZeroCache

This is the capstone project for [Rust Language UA Camp](https://github.com/rust-lang-ua/rustcamp).

**High-performance embedded key-value store with full-text search**

ZeroCache is a fast cache server with full-text search capabilities that combines the simplicity of key-value storage with powerful full-text search capabilities. Built for speed and ease of use.

## Features

- 🚀 **Blazing Fast**: Microsecond read/write operations
- 🔍 **Full-Text Search**: Tantivy inverted indexes with BM25 ranking
- 📦 **Zero Dependencies**: Single binary, no external services
- 🔄 **ACID Compliant**: Crash-safe with guaranteed data consistency
- 🎯 **Simple REST API**: JSON over HTTP, no complex protocols
- 🛡️ **Smart Access**: Public reads (rate-limited), protected writes (IP-based)
- 📊 **Flexible Schema**: Add fields anytime, index what you need
- ⚡ **Segment-Based Indexing**: Incremental updates, no full reindex needed

---

## Access Control Architecture

**ZeroCache is designed for fast public data access with protected admin operations.**

### Public Read Access (Any IP, Rate Limited)
Any IP address can query data, but limited to prevent abuse:
- **GET requests** - Search products, filter by category, get items
- Rate limit: `rate_limit_per_second: 10` (default, configurable)

**Use case:** E-commerce website visitors searching products from any device/location.

### Admin Operations (IP Restricted, No Rate Limit)
Only trusted IPs can modify data and access system info:
- **POST** - Insert/update data
- **DELETE** - Delete items/collections
- **PUT** - Modify configuration
- **System endpoints** - /status, /settings, /trees, /compact, /purge

**Configure in `settings.json`:**
```json
{
  "allowed_ips": ["127.0.0.1", "192.168.1.*", "10.0.0.5"],
  "rate_limit_per_second": 10
}
```

**Wildcard support:** Use `*` for subnet matching (e.g., `192.168.*`)

### Why This Design?

✅ **Admin** can quickly bulk-load data from backend systems (no rate limit)  
✅ **Public users** get fast product queries (rate limited to prevent abuse)  
✅ **No authentication** overhead = maximum speed  
✅ **No encryption** overhead = raw performance  
✅ **Simple IP whitelist** = easy security model

**Example workflow:**
1. Admin (from `127.0.0.1`) uploads 100k products via POST → unrestricted, fast
2. Website visitors query products via GET → 10 req/sec per IP, still very fast
3. Admin deletes old products via DELETE → unrestricted

---

## Quick Start

### Installation

```bash
cargo build --release
./target/release/zerocache
```

Server starts on `http://127.0.0.1:8080`

### Basic Usage

**Insert data:**
```bash
curl -X POST http://127.0.0.1:8080/data/products \
  -H "Content-Type: application/x-ndjson" \
  -H "X-Upsert-Field: objectID,name,description,category,price:f64" \
  -d '{"objectID":"1","name":"Wireless Mouse","description":"Ergonomic wireless mouse with adjustable DPI","category":"Electronics","price":29.99}
{"objectID":"2","name":"Mechanical Keyboard","description":"RGB backlit mechanical keyboard","category":"Electronics","price":89.99}
{"objectID":"3","name":"Running Shoes","description":"Lightweight running shoes with cushioning","category":"Sports","price":59.99}
{"objectID":"4","name":"Coffee Maker","description":"Automatic coffee maker with timer","category":"Home Appliances","price":49.99}
{"objectID":"5","name":"Bluetooth Speaker","description":"Portable waterproof Bluetooth speaker","category":"Electronics","price":39.99}
{"objectID":"6","name":"Yoga Mat","description":"Non-slip yoga mat, 6mm thick","category":"Sports","price":24.99}
{"objectID":"7","name":"Electric Kettle","description":"Fast-boiling electric kettle 1.7L","category":"Home Appliances","price":19.99}
{"objectID":"8","name":"Gaming Headset","description":"Over-ear headset with noise cancellation","category":"Electronics","price":69.99}
{"objectID":"9","name":"Dumbbell Set","description":"Adjustable dumbbell set 5-50 lbs","category":"Sports","price":99.99}
{"objectID":"10","name":"Blender","description":"High-speed blender for smoothies","category":"Home Appliances","price":34.99}'
```

**Get all items:**
```bash
curl http://127.0.0.1:8080/data/products
```

**Search by field:**
```bash
curl http://127.0.0.1:8080/data/products?category=Electronics
```

**Full-text search:**
```bash
curl http://127.0.0.1:8080/data/products?q=wireless
```

---

## API Reference

### All Endpoints

| Method | URL | Access | Description |
|--------|-----|--------|-------------|
| **GET** | `http://127.0.0.1:8080/data/products` | Public (rate limited) | Get all items in collection |
| **GET** | `http://127.0.0.1:8080/data/products?objectID=1` | Public (rate limited) | Get item by primary key |
| **GET** | `http://127.0.0.1:8080/data/products?category=Electronics` | Public (rate limited) | Filter by indexed field |
| **GET** | `http://127.0.0.1:8080/data/products?q=mouse` | Public (rate limited) | Full-text search |
| **GET** | `http://127.0.0.1:8080/data/products?limit=50&offset=100` | Public (rate limited) | Pagination |
| **GET** | `http://127.0.0.1:8080/data/products?sort_by=price&sort_order=desc` | Public (rate limited) | Sorting |
| **POST** | `http://127.0.0.1:8080/data/products` | Admin only | Insert/update items |
| **GET** | `http://127.0.0.1:8080/trees` | Admin only | List all collections |
| **GET** | `http://127.0.0.1:8080/status` | Admin only | Server health and metrics |
| **GET** | `http://127.0.0.1:8080/settings` | Admin only | Get current settings |
| **PUT** | `http://127.0.0.1:8080/settings` | Admin only | Update settings |
| **DELETE** | `http://127.0.0.1:8080/data/products?objectID=1` | Admin only | Delete item by primary key |
| **DELETE** | `http://127.0.0.1:8080/data/products` + `X-Confirm-Purge: true` | Admin only | Delete entire collection |
| **DELETE** | `http://127.0.0.1:8080/compact` + `X-Confirm-Compact: true` | Admin only | Optimize database and indexes |
| **DELETE** | `http://127.0.0.1:8080/purge` + `X-Confirm-Purge: true` | Admin only | Delete ALL data |


---

## Collections

A collection is a group of JSON documents with indexed fields.

### Create/Update Items (Upsert)

```http
POST http://127.0.0.1:8080/data/products
Content-Type: application/x-ndjson
X-Upsert-Field: objectID,name,description,category,price:f64
```

**X-Upsert-Field header:**
- First field = **primary key** (must be unique, used for direct lookups)
- Other fields = **indexed fields** (searchable, filterable, and sortable)
- **Field types** (optional): Add `:type` suffix for numeric fields
    - `:u64` - Unsigned 64-bit integer (e.g., `quantity:u64`)
    - `:i64` - Signed 64-bit integer (e.g., `temperature:i64`)
    - `:f64` - 64-bit float (e.g., `price:f64`, `rating:f64`)
    - No suffix = text field (default)

**Example:**
```bash
curl -X POST http://127.0.0.1:8080/data/products \
  -H "Content-Type: application/x-ndjson" \
  -H "X-Upsert-Field: objectID,name,description,category,price:f64" \
  -d '{"objectID":"1","name":"Mouse","description":"Wireless","category":"Electronics","price":29.99}
{"objectID":"2","name":"Keyboard","description":"Mechanical","category":"Electronics","price":89.99}'
```

**Response:**
```json
{
  "collection": "products",
  "count": 2,
  "errors": 0,
  "operation": "upsert",
  "success": true
}
```

---

## Query Data

### Get by Primary Key
```bash
curl http://127.0.0.1:8080/data/products?objectID=1
```

Returns single item directly from database (fastest - ~1-5μs).

### Get by Indexed Field
```bash
curl http://127.0.0.1:8080/data/products?category=Electronics
```

Uses search index for filtering (~100μs).

### Full-Text Search
```bash
curl "http://127.0.0.1:8080/data/products?q=wireless+mouse"
```

Searches across all indexed text fields (~1-5ms).

### Multiple Filters
```bash
curl "http://127.0.0.1:8080/data/products?category=Electronics&price=29.99"
```

Combines multiple field filters (AND logic).

### Range Filters
```bash
curl "http://127.0.0.1:8080/data/products?filter_min_price=50&filter_max_price=100"
```

Numeric range filtering.

### Pagination
```bash
curl "http://127.0.0.1:8080/data/products?limit=50&offset=100"
```

- `limit`: Items per page (default: 100, max: 1000)
- `offset`: Skip N items

**Response includes:**
```json
{
  "products": [...],
  "total": 50,
  "limit": 50,
  "offset": 100,
  "query_type": "full_scan"
}
```

### Sorting
```bash
curl "http://127.0.0.1:8080/data/products?sort_by=price&sort_order=desc"
```

- `sort_by`: Field name
- `sort_order`: `asc` or `desc` (default: asc)

### Combined Query
```bash
curl "http://127.0.0.1:8080/data/products?category=Electronics&filter_min_price=50&sort_by=price&limit=20"
```

---

## Delete Operations

### Delete by Primary Key
```bash
curl -X DELETE "http://127.0.0.1:8080/data/products?objectID=1"
```

Deletes single item and updates search index.

**Response:**
```json
{
  "deleted": 1,
  "collection": "products",
  "id": "1"
}
```

### Delete Entire Collection
```bash
curl -X DELETE http://127.0.0.1:8080/data/products \
  -H "X-Confirm-Purge: true"
```

**Requires confirmation header** to prevent accidental deletion.

**Response:**
```json
{
  "message": "Deleted collection 'products'"
}
```

---

## System Operations

### Get Status
```bash
curl http://127.0.0.1:8080/status
```

Returns server health, memory usage, disk space, and performance metrics.

**Response:**
```json
{
  "status": "healthy",
  "performance": "optimal",
  "memory": {
    "resident_bytes": 45678912,
    "resident_human": "43.56 MB",
    "virtual_bytes": 123456789,
    "virtual_human": "117.74 MB"
  },
  "db": {
    "size_bytes": 12345678,
    "size_human": "11.77 MB"
  },
  "indexes": {
    "size_bytes": 8765432,
    "size_human": "8.36 MB",
    "total_segments": 5
  },
  "disk": {
    "free_bytes": 123456789,
    "free_human": "117.74 MB"
  },
  "total_collections": 3,
  "total_items": 1250,
  "uptime": {
    "seconds": 3600,
    "human": "1h 0m 0s"
  },
  "requests": {
    "total": 45678
  },
  "can_store_data": true,
  "system_processes": 245,
  "system_memory": { ... },
  "system_cpu": { ... }
}
```

### List Collections
```bash
curl http://127.0.0.1:8080/trees
```

Returns all collections with item counts and indexed fields.

**Response:**
```json
{
  "collections": [
    {
      "name": "products",
      "count": 1000,
      "indexed": ["objectID(primary, text)", "name(text)", "category(text)", "price(f64)"]
    }
  ],
  "total": 1
}
```

### Get Settings
```bash
curl http://127.0.0.1:8080/settings
```

**Response:**
```json
{
  "port": 8080,
  "allowed_ips": ["127.0.0.1"],
  "rate_limit_per_second": 10,
  "data_path": "./data",
  "index_path": "./index",
  "upsert_index_buffer": {
    "bytes": 15000000,
    "human": "14.31 MB"
  },
  "compact_index_buffer": {
    "bytes": 50000000,
    "human": "47.68 MB"
  },
  "default_scan_limit": 100,
  "max_scan_limit": 1000,
  "payload_limit": {
    "bytes": 2097152,
    "human": "2.00 MB"
  }
}
```

### Update Settings
```bash
curl -X PUT http://127.0.0.1:8080/settings \
  -H "Content-Type: application/json" \
  -d '{"compact_index_buffer": 45000000, "rate_limit_per_second": 20}'
```

Only specified fields are updated, others remain unchanged.

### Compact Database
```bash
curl -X DELETE http://127.0.0.1:8080/compact \
  -H "X-Confirm-Compact: true"
```

**When to use:**
- To reclaim disk space
- Periodically (e.g., daily/weekly)

**What it does:**
1. Flushes Sled database to disk
2. Merges Tantivy index segments into fewer files
3. Removes deleted documents from indexes
4. Optimizes for faster queries

**Response:**
```json
{
  "results": [
    "DB compacted",
    "Merged index for products",
    "No merge needed for users"
  ]
}
```

**Performance impact:** Can take 10s-5min depending on data size. Run during low traffic.

### Purge All Data
```bash
curl -X DELETE http://127.0.0.1:8080/purge \
  -H "X-Confirm-Purge: true"
```

**WARNING:** Deletes all collections and indexes. Cannot be undone.

**Response:**
```json
{
  "message": "Purged all collections and search index"
}
```

---

## Indexed Fields Explained

### Why Index Fields?

**Indexed fields** enable fast filtering and searching:
- ✅ Direct field filtering: `?category=Electronics`
- ✅ Full-text search: `?q=wireless`
- ✅ Range queries: `?filter_min_price=50`
- ❌ Non-indexed fields cannot be filtered

### How It Works

When you upsert with `X-Upsert-Field: objectID,name,category,price`:

1. **objectID** → Primary key (direct O(log n) lookup in database)
2. **name, category, price** → Indexed in Tantivy (full-text searchable)
3. **Other fields** → Stored in database but not searchable

**Example:**
```json
{"objectID":"1","name":"Mouse","category":"Electronics","color":"black","stock":50}
```

**What you can query:**
- ✅ `?objectID=1` - Direct lookup (fastest)
- ✅ `?name=Mouse` - Indexed search
- ✅ `?category=Electronics` - Indexed filter
- ❌ `?color=black` - NOT indexed, cannot filter
- ❌ `?stock=50` - NOT indexed, cannot filter

**All fields are returned** in results, but only indexed fields can be used for filtering.

### Indexing Strategy

**Index these fields:**
- Primary keys (objectID, SKU, etc.)
- Frequently filtered fields (category, brand, status)
- Search fields (name, description, tags)
- Sort fields (price, date, rating)
- Range filter fields (price, quantity)

**Don't index:**
- Rarely queried fields
- Large text blobs (full descriptions)
- Binary data
- Frequently changing fields (view_count, last_updated)

**Trade-off:** More indexes = slower writes, faster reads. Tune based on your use case.

---

## Technology Stack

### Core Technologies

**Sled** - Embedded database
- Lock-free BTreeMap
- ACID guarantees
- Zero-copy reads
- ~1μs read latency
- Crash-safe with write-ahead log

**Tantivy** - Full-text search
- Inverted index (Lucene-like)
- Fast term queries
- BM25 relevance scoring
- Efficient compression
- Segment-based architecture

**Actix-web** - HTTP server
- Async/await runtime
- Multi-threaded worker pool
- ~50k+ req/sec throughput
- Built-in middleware support

### Performance Characteristics

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Get by primary key | 1-5 μs | 200k+ ops/sec |
| Get by indexed field | 100 μs | 10k+ ops/sec |
| Full-text search | 1-5 ms | 1k+ ops/sec |
| Insert/Update | 50-200 μs | 5k+ ops/sec |
| Bulk insert (1000 items) | 50 ms | 20k items/sec |
| Delete by primary key | 50-100 μs | 10k+ ops/sec |

*Benchmarks on modern NVMe SSD, varies by data size and query complexity*

### Why These Technologies?

1. **No external dependencies** - Single binary, no database server needed
2. **Embedded = Fast** - No network overhead, direct memory access
3. **Rust = Safe & Fast** - Memory safety without garbage collection overhead
4. **ACID compliance** - Data consistency guaranteed, even on crash
5. **Scalable** - Handles millions of documents efficiently on single node
6. **Simple deployment** - Copy binary and run, no configuration needed

---

## Configuration

Default `settings.json` (auto-created on first run):

```json
{
  "port": 8080,
  "allowed_ips": ["127.0.0.1"],
  "rate_limit_per_second": 10,
  "data_path": "./data",
  "index_path": "./index",
  "upsert_index_buffer": 15000000,
  "compact_index_buffer": 50000000,
  "default_scan_limit": 100,
  "max_scan_limit": 1000,
  "payload_limit": 2097152
}
```

### Key Settings

| Setting | Description | Unit | Default |
|---------|-------------|------|---------|
| `port` | HTTP server port | - | 8080 |
| `allowed_ips` | IP whitelist for admin operations | - | ["127.0.0.1"] |
| `rate_limit_per_second` | Max GET requests per IP per second | req/sec | 10 |
| `data_path` | Sled database directory | - | ./data |
| `index_path` | Tantivy indexes directory | - | ./index |
| `upsert_index_buffer` | Memory for index writes | bytes | 15 MB |
| `compact_index_buffer` | Memory for compaction | bytes | 50 MB |
| `default_scan_limit` | Default items per query | items | 100 |
| `max_scan_limit` | Maximum items per query | items | 1000 |
| `payload_limit` | Max HTTP request body size | bytes | 2 MB |


---

## Use Cases

### ✅ Perfect For:

- **E-commerce product catalogs** - Fast product search and filtering
- **Content management** - Articles, blogs with full-text search
- **API response cache** - Cache expensive API calls with TTL
- **Session storage** - Persistent sessions with fast lookup
- **Real-time analytics** - Temporary data aggregation
- **Search-as-you-type** - Instant search suggestions
- **Inventory systems** - SKU lookups and stock filtering
- **Testing/development** - Drop-in database for prototypes

### ❌ Not Ideal For:

- **Complex joins** - Use relational database (PostgreSQL, MySQL)
- **Very large datasets** - >100GB may need distributed system
- **Frequent schema changes** - Requires re-indexing
- **Transactional workflows** - No multi-collection transactions

---

## Maintenance

### Monitoring

Check `/status` endpoint regularly for these metrics:

| Metric | Action Needed When |
|--------|-------------------|
| `indexes.total_segments` | > 10 segments → Run `/compact` |
| `disk.free_bytes` | < 10% free → Clean up or add storage |
| `memory.resident_bytes` | Growing continuously → Check for leaks |
| `can_store_data` | `false` → Disk full, urgent action needed |

### Optimization

**Weekly maintenance:**
```bash
curl -X DELETE http://127.0.0.1:8080/compact \
  -H "X-Confirm-Compact: true"
```

**Performance tips:**
1. Run compaction during low-traffic hours
2. Monitor segment count - more segments = slower search
3. Use specific filters instead of full scans
4. Limit query results with `limit` parameter

---

## Troubleshooting

### Slow Queries?

**Check segments:**
```bash
curl http://127.0.0.1:8080/status | grep total_segments
```
If >10 segments, run compact.

**Solutions:**
- Run `DELETE http://127.0.0.1:8080/compact`
- Reduce indexed fields
- Add more specific filters to queries
- Use primary key lookups when possible

### High Memory Usage?

**Check current usage:**
```bash
curl http://127.0.0.1:8080/status | grep memory
```

**Solutions:**
- Reduce `upsert_index_buffer` and `compact_index_buffer` in settings
- Run compaction to free memory
- Restart server if memory leak suspected

### Disk Space Full?

**Check available space:**
```bash
curl http://127.0.0.1:8080/status | grep disk
```

**Solutions:**
- Run compaction to reclaim space
- Delete old collections
- Clear unnecessary data from other applications

### Search Not Working?

**Verify field is indexed:**
```bash
curl http://127.0.0.1:8080/trees
```

Check if field appears in `indexed` array.

**Solutions:**
- Ensure field was in `X-Upsert-Field` header during POST
- Re-insert data with correct header
- Check field name spelling in query

### Rate Limit Errors (HTTP 429)?

**Increase limit for legitimate traffic:**
```bash
curl -X PUT http://127.0.0.1:8080/settings \
  -H "Content-Type: application/json" \
  -d '{"rate_limit_per_second": 50}'
```

Or add IP to `allowed_ips` to bypass rate limiting.

---

## Testing

Run integration tests:
```bash
# Start server
cargo run

# In another terminal
cargo test
```

All 14 tests cover:
- CRUD operations
- Search queries (full-text, filters, ranges)
- Pagination and sorting
- Delete operations (by key, entire collection)
- System endpoints (status, settings, trees)
- Access control validation
- Rate limiting

---

## Example: E-commerce Integration

### Backend Setup (Admin IP: 127.0.0.1)

**1. Bulk import products from main database:**
```bash
# Export from PostgreSQL/MySQL as NDJSON
# Import to ZeroCache
curl -X POST http://127.0.0.1:8080/data/products \
  -H "Content-Type: application/x-ndjson" \
  -H "X-Upsert-Field: sku,name,description,category,brand,price" \
  --data-binary @products.ndjson
```

**2. Update single product:**
```bash
curl -X POST http://127.0.0.1:8080/data/products \
  -H "Content-Type: application/x-ndjson" \
  -H "X-Upsert-Field: sku,name,category,price" \
  -d '{"sku":"ABC123","name":"Updated Name","category":"Electronics","price":99.99}'
```

### Frontend Queries (Any IP, Rate Limited)

**Search products:**
```javascript
// Full-text search
fetch('http://cache.example.com:8080/data/products?q=laptop')
  .then(r => r.json())
  .then(data => console.log(data.products));

// Filter by category
fetch('http://cache.example.com:8080/data/products?category=Electronics&limit=20')
  .then(r => r.json())
  .then(data => displayProducts(data.products));

// Price range + sort
fetch('http://cache.example.com:8080/data/products?filter_min_price=100&filter_max_price=500&sort_by=price&sort_order=asc')
  .then(r => r.json())
  .then(data => displayProducts(data.products));
```

### Architecture

```
┌─────────────────┐
│  PostgreSQL/    │  (Source of truth)
│  MySQL Database │
└────────┬────────┘
         │ Sync/Export
         ▼
┌─────────────────┐
│   ZeroCache     │  (Fast search cache)
│   127.0.0.1     │  - Admin access only
└────────┬────────┘
         │ Public queries
         ▼
┌─────────────────┐
│   Website       │  (Any IP)
│   Users         │  - Rate limited GET
└─────────────────┘
```

**Benefits:**
- Main DB handles writes and complex queries
- ZeroCache handles fast product search
- 10-100x faster than SQL for search queries
- No load on main database for searches

---

**ZeroCache** - Zero complexity, maximum performance! 🚀
