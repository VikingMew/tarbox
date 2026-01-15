# Security Policy

## Supported Versions

We release patches for security vulnerabilities. Currently supported versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take the security of Tarbox seriously. If you believe you have found a security vulnerability, please report it to us as described below.

### Where to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via creating a private security advisory on GitHub

### What to Include

Please include the following information in your report:

- Type of issue (e.g., buffer overflow, SQL injection, cross-site scripting, etc.)
- Full paths of source file(s) related to the manifestation of the issue
- The location of the affected source code (tag/branch/commit or direct URL)
- Any special configuration required to reproduce the issue
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if possible)
- Impact of the issue, including how an attacker might exploit the issue

### What to Expect

- You will receive an acknowledgment of your report within 48 hours
- We will send a more detailed response within 7 days indicating the next steps
- We will keep you informed about the progress towards a fix and full announcement
- We may ask for additional information or guidance

## Security Considerations

### Multi-Tenancy

Tarbox implements strict multi-tenant isolation:

- **Database Level**: Every query includes `tenant_id` in WHERE clauses
- **Filesystem Level**: Path resolution enforces tenant boundaries
- **Audit Trail**: All operations are logged with tenant context

### Authentication and Authorization

- **PostgreSQL Authentication**: Secure database connection with strong credentials
- **POSIX Permissions**: Standard UNIX permission model enforced
- **Tenant Isolation**: Complete data separation between tenants

### Data Protection

- **Encryption at Rest**: Configure PostgreSQL with encryption (recommended)
- **Encryption in Transit**: Use SSL/TLS for database connections
- **Audit Logging**: All file operations logged for compliance

### Native Mounts

Native filesystem mounts bypass PostgreSQL and require special attention:

- **Read-Only by Default**: System directories should be mounted as `ro`
- **Path Validation**: All paths are validated before access
- **Tenant Separation**: Tenant-specific paths use `{tenant_id}` variables
- **Audit Logging**: Native mount operations are logged separately

### Best Practices

1. **Database Security**
   - Use strong passwords for PostgreSQL
   - Enable SSL/TLS for database connections
   - Regularly update PostgreSQL to latest security patches
   - Configure appropriate `pg_hba.conf` rules

2. **Filesystem Security**
   - Run FUSE mount with appropriate user permissions
   - Use read-only mounts for system directories
   - Regularly review audit logs for suspicious activity

3. **Network Security**
   - Use firewall rules to restrict database access
   - Deploy in private networks when possible
   - Use VPN or SSH tunnels for remote access

4. **Operational Security**
   - Keep Tarbox updated to latest version
   - Monitor audit logs regularly
   - Implement backup and disaster recovery procedures
   - Test recovery procedures periodically

## Known Security Limitations

### Alpha Software

Tarbox is currently in early development (MVP phase). It has not undergone:

- Comprehensive security audits
- Penetration testing
- Production hardening at scale

**Do not use in production environments handling sensitive data until version 1.0.**

### Potential Attack Vectors

Areas requiring additional security review:

1. **FUSE Interface**: User-space filesystem could have privilege escalation risks
2. **SQL Injection**: While using prepared statements, review is ongoing
3. **Path Traversal**: Path validation logic needs thorough testing
4. **Resource Exhaustion**: Rate limiting and quotas not yet implemented
5. **Native Mounts**: Direct filesystem access bypasses database controls

## Security Updates

Security updates will be released as:

- **Critical**: Immediate patch release
- **High**: Patch within 7 days
- **Medium**: Patch in next minor release
- **Low**: Patch in next major release

All security updates will be announced via:

- GitHub Security Advisories
- Release notes
- Project documentation

## Vulnerability Disclosure Policy

We follow coordinated vulnerability disclosure:

1. Reporter notifies us privately
2. We confirm the vulnerability
3. We develop and test a fix
4. We release the fix
5. We publicly disclose the vulnerability (after fix is available)

Typical disclosure timeline: 90 days from initial report

## Bug Bounty

We do not currently have a bug bounty program. This may change as the project matures.

## Credits

We thank the following security researchers for responsible disclosure:

- (List will be updated as vulnerabilities are reported and fixed)

---

For any questions about this policy, please contact: security@tarbox.dev
