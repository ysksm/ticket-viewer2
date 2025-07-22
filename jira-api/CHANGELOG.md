# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2024-12-XX

### Added

#### Core API Client Features
- JIRA REST API v3 client implementation
- Support for Basic and Bearer authentication methods
- HTTP client with proper error handling and timeout support
- Comprehensive data models for JIRA entities (Issue, Project, User, etc.)

#### Supported API Endpoints
- `/rest/api/3/search` - Issue search with JQL query support
- `/rest/api/3/project` - Project listing and details
- `/rest/api/3/priority` - Priority metadata
- `/rest/api/3/issuetype` - Issue type metadata
- `/rest/api/3/field` - Field metadata including custom fields
- `/rest/api/3/statuscategory` - Status category metadata
- `/rest/api/3/users/search` - User search functionality

#### Data Persistence
- Abstract `PersistenceStore` trait for pluggable storage backends
- JSON storage implementation with gzip compression support
- DuckDB storage implementation with SQL query capabilities
- Advanced filtering and sorting options
- Storage statistics and performance metrics
- Issue filtering by project, status, priority, date ranges, and custom criteria

#### Synchronization Features
- `SyncService` for incremental and full data synchronization
- Time-based filtering with hour-level granularity
- Automatic deduplication of Issue records
- Sync history tracking with configurable retention
- Sync statistics including success rates and performance metrics
- Support for concurrent sync operations with configurable limits
- JQL generation for time-based queries with excluded issue keys

#### Change History Management
- Complete changelog parsing from JIRA's `expand=changelog` parameter
- Detailed change tracking for all Issue fields
- History storage in DuckDB with optimized schema
- History filtering by date range, change type, author, and issue keys
- History statistics and analytics
- Support for tracking status changes, assignee changes, field updates, etc.

#### Configuration Management
- `ConfigStore` trait for configuration persistence
- File-based configuration storage with JSON format
- Environment variable configuration support
- Configuration validation and error handling
- Support for storing authentication credentials and filter preferences

#### Memory Management
- Lazy loading support for large datasets
- Memory-efficient streaming for bulk operations
- Configurable memory pools and garbage collection
- Issue detail loading with multiple resolution levels
- Memory usage monitoring and optimization

#### Time-Based Filtering
- Flexible time filter creation (last N hours/days, custom ranges)
- Time chunk splitting for large date ranges
- JQL condition generation for time-based queries
- Support for filtering by created date, updated date, or both
- Configurable time granularity (hour-based chunking)

#### Error Handling
- Comprehensive error types using `thiserror`
- Network error handling with retry logic
- API error response parsing
- Validation errors for configuration and inputs
- Detailed error context and debugging information

### Documentation
- Complete API documentation with examples
- Comprehensive usage examples covering all major features
- Architecture and design specifications
- Development task tracking and progress monitoring
- Detailed README with quick start guide and feature overview

### Examples
- `basic_usage.rs` - Simple client usage and authentication
- `search_example.rs` - Advanced Issue search with various JQL queries
- `project_example.rs` - Project metadata retrieval
- `persistence_example.rs` - Data storage and retrieval examples
- `sync_example.rs` - Complete synchronization workflow
- `history_example.rs` - Change history analysis
- `config_store_example.rs` - Configuration management
- `hybrid_integration_example.rs` - Mock and real API testing

### Testing
- Unit tests for all core components
- Integration tests with real JIRA API support
- Performance tests for large dataset handling
- Error scenario tests for robustness validation
- Concurrency tests for thread safety verification
- End-to-end tests covering complete workflows

### Development Features
- Test-driven development (TDD) approach throughout
- Comprehensive CI/CD setup preparation
- Code formatting with `rustfmt`
- Linting with `clippy`
- Documentation generation with `cargo doc`

### Dependencies
- `reqwest` - HTTP client with JSON and TLS support
- `serde` / `serde_json` - Serialization framework
- `tokio` - Async runtime
- `thiserror` - Error handling
- `chrono` - Date and time handling
- `async-trait` - Async trait support
- `duckdb` - Embedded SQL database
- `flate2` - Gzip compression
- `url` - URL parsing and validation
- `base64` - Authentication encoding
- `dirs` - Cross-platform directory handling

## [0.0.1] - Initial Development

### Added
- Project initialization
- Basic project structure
- Development environment setup
- Initial dependency configuration

---

## Development Notes

This project follows Test-Driven Development (TDD) principles with the Red-Green-Refactor cycle:

1. **Red**: Write failing tests first
2. **Green**: Implement minimal code to pass tests
3. **Refactor**: Improve code quality while maintaining functionality

### Version Strategy

- **Major version** (1.x.x): Breaking API changes
- **Minor version** (0.x.x): New features, backward compatible
- **Patch version** (0.0.x): Bug fixes, backward compatible

### Release Process

1. Update version in `Cargo.toml`
2. Update this `CHANGELOG.md` with new features and changes
3. Run full test suite (`cargo test`)
4. Generate updated documentation (`cargo doc`)
5. Create git tag (`git tag vX.Y.Z`)
6. Publish to crates.io (`cargo publish`)

### Contribution Guidelines

When contributing to this project:

1. Follow TDD principles - tests first, then implementation
2. Update relevant documentation and examples
3. Add changelog entries for user-visible changes
4. Ensure all tests pass and no clippy warnings
5. Update version numbers appropriately

### Breaking Changes Policy

Breaking changes will only be introduced in major version releases (1.x.x), with the following exceptions:

- Security fixes may introduce breaking changes in minor releases
- Bug fixes that correct unintended behavior may break code that relied on the bug
- Changes to internal/private APIs that were not intended for public use

All breaking changes will be clearly documented in this changelog with migration guidance where applicable.