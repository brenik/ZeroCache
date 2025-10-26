# ZeroCache
This is the capstone project for [Rust Language UA Camp](https://github.com/rust-lang-ua/rustcamp).

## Topic Description
ZeroCache is a simple in-memory cache system in Rust, for practicing concurrency and data structures.
A high-performance in-memory JSON cache server with HTTP API, designed for applications requiring fast data access with persistent storage.

## Planned Features

### Core Functionality
- **In-Memory Storage**: JSON data collections using concurrent HashMap with lock-free access
- **HTTP REST API**: Complete CRUD operations (POST/GET/DELETE) on collections and items
- **Memory-Mapped Persistence**: Data survives server restarts using efficient mmap with MessagePack serialization
- **Smart Upsert Operations**: Configurable field-based insert/update logic
- **Nested Data Handling**: Support for complex JSON structures in updates and deletions

### Advanced Operations
- **Query Filtering**: Filter collection items by field values
- **Memory Management**: Configurable limits, size tracking, and automatic overflow rejection
- **Bulk Operations**: Process multiple items in single requests
- **Cache Optimization**: Compact endpoint for storage file defragmentation

### Production Features
- **Rate Limiting**: Per-IP request throttling using actix-governor
- **Access Control**: IP-based restrictions for write operations
- **Health Monitoring**: Comprehensive status endpoint for cache health and usage metrics
- **Settings Management**: Runtime configuration with validation and persistence
- **Graceful Startup**: Asynchronous cache loading with degraded mode handling

## Technology Stack

- **Rust** - Memory safety and performance
- **Actix-web** - Async HTTP server framework
- **Flurry** - Lock-free concurrent HashMap
- **Serde/serde_json** - JSON serialization and handling
- **rmp-serde/Bincode** - Efficient binary serialization formats
- **memmap2** - Memory-mapped file operations
- **Sysinfo** - System memory information
- **actix-governor** - Rate limiting middleware
- **env_logger** - Structured logging

## API Overview

### Data Operations
- `GET /data` - Retrieve all collections
- `GET /data/{collection}` - Get collection items (with optional filtering)
- `POST /data` - Insert/upsert data into collections
- `DELETE /data/{collection}` - Delete items from collection
- `DELETE /data/purge` - Remove all data (requires confirmation header)

### Management
- `GET /status` - System health and collection statistics
- `GET /settings` - Current configuration
- `PUT /settings` - Update configuration
- `POST /compact` - Optimize storage files

## Development Status

🚧 **Active Development** - Core functionality implemented, seeking feedback and collaboration

## Contributing

This project is part of [Rust Language UA Camp](https://github.com/rust-lang-ua/rustcamp).
All feedback, suggestions, and contributions are welcome!