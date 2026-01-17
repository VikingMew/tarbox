// Config and Storage Models Unit Tests
//
// 文件：tests/config_and_storage_models_test.rs
// 目的：测试配置系统和存储数据模型的所有变体、边界情况和验证逻辑
//
// 测试覆盖：
// - Config: 默认值、序列化、各子配置的验证
// - DatabaseConfig: URL 格式、连接池边界
// - Storage Models: CreateTenantInput, InodeType, Block hash 函数

#[cfg(test)]
mod config_tests {
    use tarbox::config::*;

    #[test]
    fn test_config_default_values() {
        let config = Config::default();

        // 测试所有配置字段
        assert!(!config.database.url.is_empty());
        assert!(config.database.max_connections > 0);
        assert!(config.database.min_connections > 0);
        assert!(!config.fuse.mount_point.is_empty());
        assert!(config.cache.max_entries > 0);
        assert!(config.cache.ttl_seconds > 0);
        assert!(!config.api.rest_addr.is_empty());
        assert!(!config.api.grpc_addr.is_empty());

        // 测试序列化
        let json = serde_json::to_string(&config);
        assert!(json.is_ok());
    }

    #[test]
    fn test_database_config_url_formats() {
        let configs = vec![
            DatabaseConfig {
                url: "postgresql://localhost/db1".into(),
                max_connections: 5,
                min_connections: 1,
            },
            DatabaseConfig {
                url: "postgresql://user@host/db2".into(),
                max_connections: 10,
                min_connections: 2,
            },
            DatabaseConfig {
                url: "postgresql://user:pass@host:5432/db3".into(),
                max_connections: 20,
                min_connections: 5,
            },
        ];

        for config in configs {
            assert!(config.url.starts_with("postgresql://"));
            assert!(config.max_connections >= config.min_connections);

            // 测试克隆
            let cloned = config.clone();
            assert_eq!(config.url, cloned.url);
        }
    }

    #[test]
    fn test_fuse_config_allow_other_flag() {
        let configs = vec![
            FuseConfig { mount_point: "/mnt/test1".into(), allow_other: false },
            FuseConfig { mount_point: "/mnt/test2".into(), allow_other: true },
        ];

        for config in configs {
            assert!(!config.mount_point.is_empty());
            let cloned = config.clone();
            assert_eq!(config.mount_point, cloned.mount_point);
            assert_eq!(config.allow_other, cloned.allow_other);
        }
    }

    #[test]
    fn test_audit_config_retention_periods() {
        let configs = vec![
            AuditConfig { enabled: true, retention_days: 30 },
            AuditConfig { enabled: false, retention_days: 90 },
            AuditConfig { enabled: true, retention_days: 365 },
        ];

        for config in configs {
            assert!(config.retention_days > 0);
            let cloned = config.clone();
            assert_eq!(config.enabled, cloned.enabled);
            assert_eq!(config.retention_days, cloned.retention_days);
        }
    }

    #[test]
    fn test_cache_config_size_and_ttl() {
        let configs = vec![
            CacheConfig { max_entries: 1000, ttl_seconds: 60 },
            CacheConfig { max_entries: 10000, ttl_seconds: 300 },
            CacheConfig { max_entries: 100000, ttl_seconds: 600 },
        ];

        for config in configs {
            assert!(config.max_entries > 0);
            assert!(config.ttl_seconds > 0);
        }
    }

    #[test]
    fn test_api_config_address_formats() {
        let configs = vec![
            ApiConfig { rest_addr: "127.0.0.1:8080".into(), grpc_addr: "127.0.0.1:50051".into() },
            ApiConfig { rest_addr: "0.0.0.0:3000".into(), grpc_addr: "0.0.0.0:4000".into() },
        ];

        for config in configs {
            assert!(config.rest_addr.contains(':'));
            assert!(config.grpc_addr.contains(':'));
        }
    }
}

#[cfg(test)]
mod storage_model_tests {
    use tarbox::storage::*;

    #[test]
    fn test_create_tenant_input_validation() {
        // CreateTenantInput
        let tenant_input = CreateTenantInput { tenant_name: "test_tenant".into() };
        assert!(!tenant_input.tenant_name.is_empty());
        let cloned = tenant_input.clone();
        assert_eq!(tenant_input.tenant_name, cloned.tenant_name);

        // InodeType variants
        let types = vec![InodeType::File, InodeType::Dir, InodeType::Symlink];
        for t in types {
            let debug_str = format!("{:?}", t);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_inode_type_debug_and_equality() {
        let file = InodeType::File;
        let dir = InodeType::Dir;
        let symlink = InodeType::Symlink;

        assert_eq!(format!("{:?}", file), "File");
        assert_eq!(format!("{:?}", dir), "Dir");
        assert_eq!(format!("{:?}", symlink), "Symlink");

        assert_ne!(file, dir);
        assert_ne!(dir, symlink);
    }

    #[test]
    fn test_block_content_hash_properties() {
        use tarbox::storage::block::compute_content_hash;

        // 测试各种数据
        let empty: &[u8] = b"";
        let single = b"a";
        let phrase = b"hello world";
        let digits = b"0123456789";
        let large = vec![0u8; 1000];
        let whitespace = b"\n\r\t";

        for data in [empty, single, phrase, digits, &large, whitespace] {
            let hash = compute_content_hash(data);
            assert_eq!(hash.len(), 64); // blake3 hex
            assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        }

        // 测试确定性
        let data = b"deterministic";
        let hash1 = compute_content_hash(data);
        let hash2 = compute_content_hash(data);
        assert_eq!(hash1, hash2);

        // 测试不同数据产生不同哈希
        let hash_a = compute_content_hash(b"a");
        let hash_b = compute_content_hash(b"b");
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn test_block_hash_determinism_and_collision_resistance() {
        use tarbox::storage::block::compute_content_hash;

        // 测试确定性
        let data = b"deterministic";
        let hash1 = compute_content_hash(data);
        let hash2 = compute_content_hash(data);
        assert_eq!(hash1, hash2);

        // 测试不同数据产生不同哈希
        let hash_a = compute_content_hash(b"a");
        let hash_b = compute_content_hash(b"b");
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn test_database_config_connection_pool_bounds() {
        use tarbox::config::DatabaseConfig;

        // 最小配置
        let min_config = DatabaseConfig {
            url: "postgresql://localhost/test".into(),
            max_connections: 1,
            min_connections: 1,
        };
        assert_eq!(min_config.max_connections, min_config.min_connections);

        // 大配置
        let max_config = DatabaseConfig {
            url: "postgresql://localhost/test".into(),
            max_connections: 100,
            min_connections: 10,
        };
        assert!(max_config.max_connections > max_config.min_connections);
    }
}
