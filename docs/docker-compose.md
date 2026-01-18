# Docker Compose Guide

This guide explains how to use the provided `docker-compose.yml` for Tarbox development and testing.

## Services Overview

The Docker Compose configuration provides four services:

- **postgres**: Main PostgreSQL database for development
- **postgres-test**: Separate PostgreSQL database for running tests
- **tarbox-cli**: Pre-built Tarbox CLI container for manual testing
- **pgadmin**: Optional web-based PostgreSQL administration tool

## Quick Start

### Start Development Database

```bash
# Start only the main PostgreSQL database
docker-compose up -d postgres

# Verify it's running
docker-compose ps
```

The main database will be available at `localhost:5432` with:
- Database: `tarbox`
- Username: `tarbox`
- Password: `tarbox123`

### Start Test Database

```bash
# Start the test database (required for E2E tests)
docker-compose up -d postgres-test

# Verify it's running
docker-compose ps
```

The test database will be available at `localhost:5433` with:
- Database: `tarbox_test`
- Username: `tarbox_test`
- Password: `test123`

## Running Tests

### Unit Tests (No Database Required)

```bash
# Run unit tests only - fast, no dependencies
cargo test --lib
```

### Integration Tests with Mocks (No Database Required)

```bash
# Run unit + integration tests with mockall
cargo test
```

### E2E Tests (Requires Test Database)

```bash
# Start test database
docker-compose up -d postgres-test

# Wait for database to be ready (about 5 seconds)
sleep 5

# Run all tests including E2E
DATABASE_URL=postgres://tarbox_test:test123@localhost:5433/tarbox_test cargo test --all-targets

# Or use the environment variable
export DATABASE_URL=postgres://tarbox_test:test123@localhost:5433/tarbox_test
cargo test --all-targets
```

## Using the CLI Container

The `tarbox-cli` service provides a pre-built container for manual testing:

```bash
# Build and start the CLI container
docker-compose up -d tarbox-cli

# Execute commands inside the container
docker-compose exec tarbox-cli tarbox tenant create alice
docker-compose exec tarbox-cli tarbox tenant list

# Interactive shell
docker-compose exec tarbox-cli bash
```

**Note**: The CLI container connects to the main `postgres` service automatically via Docker networking.

## Database Administration with pgAdmin

pgAdmin is an optional web-based tool for inspecting and managing the PostgreSQL databases.

### Start pgAdmin

```bash
# Start pgAdmin (uses 'tools' profile)
docker-compose --profile tools up -d pgadmin

# Access at http://localhost:5050
# Email: admin@tarbox.local
# Password: admin123
```

### Connect to Databases in pgAdmin

1. Open http://localhost:5050 in your browser
2. Login with credentials above
3. Right-click "Servers" → "Register" → "Server"
4. Configure connection:

**Main Database:**
- Name: Tarbox Dev
- Host: postgres
- Port: 5432
- Database: tarbox
- Username: tarbox
- Password: tarbox123

**Test Database:**
- Name: Tarbox Test
- Host: postgres-test
- Port: 5432
- Database: tarbox_test
- Username: tarbox_test
- Password: test123

## Data Persistence

Database data is stored in Docker volumes:

- `postgres_data`: Main database data
- `postgres_test_data`: Test database data
- `pgadmin_data`: pgAdmin settings and connections

### Clear Database Data

```bash
# Stop all services
docker-compose down

# Remove volumes to clear all data
docker-compose down -v

# Or remove specific volumes
docker volume rm tarbox_postgres_data
docker volume rm tarbox_postgres_test_data
```

## Common Workflows

### Development Workflow

```bash
# 1. Start development database
docker-compose up -d postgres

# 2. Build and test locally
cargo build
cargo test

# 3. Run CLI for manual testing
cargo run -- tenant create alice
cargo run -- --tenant alice write /hello.txt "Hello World"
cargo run -- --tenant alice read /hello.txt

# 4. Stop when done
docker-compose down
```

### Testing Workflow

```bash
# 1. Start test database
docker-compose up -d postgres-test

# 2. Run full test suite
export DATABASE_URL=postgres://tarbox_test:test123@localhost:5433/tarbox_test
cargo test --all-targets

# 3. Check coverage
cargo llvm-cov --all-targets --html

# 4. Clean up
docker-compose down
```

### Container-Based Testing

```bash
# 1. Build CLI container with latest code
docker-compose build tarbox-cli

# 2. Start database and CLI
docker-compose up -d postgres tarbox-cli

# 3. Test inside container
docker-compose exec tarbox-cli tarbox tenant create bob
docker-compose exec tarbox-cli tarbox --tenant bob mkdir /workspace
docker-compose exec tarbox-cli tarbox --tenant bob ls /

# 4. Clean up
docker-compose down
```

## Troubleshooting

### Database Connection Refused

```bash
# Check if database is running
docker-compose ps

# Check logs
docker-compose logs postgres

# Restart database
docker-compose restart postgres
```

### Port Conflicts

If ports 5432 or 5433 are already in use:

```bash
# Check what's using the port
sudo lsof -i :5432
sudo lsof -i :5433

# Option 1: Stop conflicting service
sudo systemctl stop postgresql

# Option 2: Change ports in docker-compose.yml
# Edit the ports mapping:
# postgres:
#   ports:
#     - "15432:5432"  # Use different host port
```

### Test Database Not Initialized

```bash
# Recreate test database
docker-compose down postgres-test
docker volume rm tarbox_postgres_test_data
docker-compose up -d postgres-test

# Wait for initialization
sleep 10
```

### CLI Container Build Failures

```bash
# Clean build with no cache
docker-compose build --no-cache tarbox-cli

# Check build logs
docker-compose build tarbox-cli
```

## Environment Variables

You can customize the configuration using environment variables:

```bash
# Create .env file in project root
cat > .env << EOF
POSTGRES_PASSWORD=my_secure_password
POSTGRES_TEST_PASSWORD=my_test_password
PGADMIN_EMAIL=me@example.com
PGADMIN_PASSWORD=my_admin_password
EOF

# Restart services to apply changes
docker-compose down
docker-compose up -d
```

**Note**: Never commit `.env` files with real credentials to version control.

## Production Deployment

**Warning**: This docker-compose.yml is for development only. For production:

1. Use stronger passwords (generate with `openssl rand -base64 32`)
2. Don't expose database ports to host
3. Use Docker secrets instead of environment variables
4. Configure PostgreSQL with proper backups
5. Use SSL/TLS for database connections
6. Set up monitoring and alerting
7. Use managed PostgreSQL services (AWS RDS, Google Cloud SQL, etc.)

See the main README for production deployment recommendations.

## Additional Resources

- [PostgreSQL Documentation](https://www.postgresql.org/docs/)
- [Docker Compose Documentation](https://docs.docker.com/compose/)
- [pgAdmin Documentation](https://www.pgadmin.org/docs/)
- [Tarbox Development Guide](../task/README.md)
