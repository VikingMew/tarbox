// Storage and Config Model Tests
//
// 这些测试验证数据模型的所有变体、边界情况和约束

use uuid::Uuid;

#[cfg(test)]
mod inode_model_tests {
    use super::*;

    #[test]
    fn test_inode_type_enum_variants() {
        use tarbox::storage::InodeType;

        let file = InodeType::File;
        let dir = InodeType::Dir;
        let symlink = InodeType::Symlink;

        // 测试 Debug trait
        assert_eq!(format!("{:?}", file), "File");
        assert_eq!(format!("{:?}", dir), "Dir");
        assert_eq!(format!("{:?}", symlink), "Symlink");

        // 测试相等性
        assert_eq!(file, InodeType::File);
        assert_ne!(file, dir);
        assert_ne!(dir, symlink);
    }

    #[test]
    fn test_create_inode_input_field_combinations() {
        use tarbox::storage::InodeType;
        use tarbox::storage::models::CreateInodeInput;

        let tenant_id = Uuid::new_v4();

        // 测试所有字段组合
        let inputs = vec![
            CreateInodeInput {
                tenant_id,
                parent_id: None,
                name: "root".to_string(),
                inode_type: InodeType::Dir,
                mode: 0o755,
                uid: 0,
                gid: 0,
            },
            CreateInodeInput {
                tenant_id,
                parent_id: Some(1),
                name: "file.txt".to_string(),
                inode_type: InodeType::File,
                mode: 0o644,
                uid: 1000,
                gid: 1000,
            },
            CreateInodeInput {
                tenant_id,
                parent_id: Some(1),
                name: "link".to_string(),
                inode_type: InodeType::Symlink,
                mode: 0o777,
                uid: 1000,
                gid: 1000,
            },
        ];

        for input in inputs {
            assert_eq!(input.tenant_id, tenant_id);
            assert!(!input.name.is_empty());
            assert!(input.mode > 0);
        }
    }
}

#[cfg(test)]
mod update_inode_tests {
    use super::*;

    #[test]
    fn test_update_inode_input_partial_updates() {
        use tarbox::storage::models::UpdateInodeInput;

        let now = chrono::Utc::now();

        // 测试各种更新组合
        let updates = vec![
            UpdateInodeInput {
                size: Some(1024),
                mode: None,
                uid: None,
                gid: None,
                atime: None,
                mtime: None,
                ctime: None,
            },
            UpdateInodeInput {
                size: None,
                mode: Some(0o755),
                uid: Some(1000),
                gid: Some(1000),
                atime: None,
                mtime: None,
                ctime: None,
            },
            UpdateInodeInput {
                size: None,
                mode: None,
                uid: None,
                gid: None,
                atime: Some(now),
                mtime: Some(now),
                ctime: Some(now),
            },
        ];

        for update in updates {
            // 验证至少有一个字段被设置
            let has_update = update.size.is_some()
                || update.mode.is_some()
                || update.uid.is_some()
                || update.gid.is_some()
                || update.atime.is_some()
                || update.mtime.is_some()
                || update.ctime.is_some();
            assert!(has_update);
        }
    }
}

#[cfg(test)]
mod block_model_tests {
    use super::*;

    #[test]
    fn test_create_block_input_data_sizes() {
        use tarbox::storage::models::CreateBlockInput;

        let tenant_id = Uuid::new_v4();

        let inputs = vec![
            CreateBlockInput { tenant_id, inode_id: 1, block_index: 0, data: vec![] },
            CreateBlockInput { tenant_id, inode_id: 2, block_index: 1, data: vec![0u8; 4096] },
            CreateBlockInput {
                tenant_id,
                inode_id: 3,
                block_index: 2,
                data: b"hello world".to_vec(),
            },
        ];

        for (idx, input) in inputs.iter().enumerate() {
            assert_eq!(input.tenant_id, tenant_id);
            assert_eq!(input.block_index, idx as i32);
        }
    }

    #[test]
    fn test_block_content_hash_all_data_types() {
        use tarbox::storage::block::compute_content_hash;

        // 测试所有边界情况
        let test_cases = vec![
            (vec![], "empty"),
            (vec![0], "zero"),
            (vec![255], "max_byte"),
            (vec![0u8; 4096], "block_size"),
            (b"ASCII".to_vec(), "ascii"),
            ("UTF-8文本".as_bytes().to_vec(), "utf8"),
        ];

        for (data, label) in test_cases {
            let hash = compute_content_hash(&data);
            assert_eq!(hash.len(), 64, "Failed for {}", label);
            assert!(hash.chars().all(|c| c.is_ascii_hexdigit()), "Failed for {}", label);
        }
    }
}

#[cfg(test)]
mod tenant_model_tests {
    use super::*;

    #[test]
    fn test_create_tenant_input_name_variations() {
        use tarbox::storage::models::CreateTenantInput;

        let long_name = "a".repeat(100);
        let cases = vec![
            "a",
            "tenant_123",
            "test-tenant",
            "UPPERCASE",
            "mixed_Case-123",
            long_name.as_str(),
        ];

        for name in cases {
            let input = CreateTenantInput { tenant_name: name.to_string() };
            assert_eq!(input.tenant_name, name);

            let cloned = input.clone();
            assert_eq!(cloned.tenant_name, name);
        }
    }

    #[test]
    fn test_block_compute_hash_comprehensive() {
        use tarbox::storage::block::compute_content_hash;

        // 测试所有边界情况
        let test_cases = vec![
            (vec![], "empty"),
            (vec![0], "single_zero"),
            (vec![255], "single_max"),
            (vec![0u8; 1], "one_byte"),
            (vec![0u8; 4096], "one_block"),
            (vec![0u8; 8192], "two_blocks"),
            (b"ASCII text".to_vec(), "ascii"),
            ("UTF-8 文本 🔥".as_bytes().to_vec(), "utf8"),
            (vec![0, 1, 2, 3, 4, 5], "sequence"),
            ((0..=255u8).collect::<Vec<_>>(), "all_bytes"),
        ];

        for (data, label) in test_cases {
            let hash = compute_content_hash(&data);

            // 验证哈希属性
            assert_eq!(hash.len(), 64, "Hash length mismatch for {}", label);
            assert!(hash.chars().all(|c| c.is_ascii_hexdigit()), "Non-hex char in {}", label);
            assert_eq!(hash, hash.to_lowercase(), "Hash not lowercase for {}", label);

            // 验证确定性
            let hash2 = compute_content_hash(&data);
            assert_eq!(hash, hash2, "Non-deterministic hash for {}", label);
        }

        // 验证不同数据产生不同哈希
        let hash1 = compute_content_hash(b"data1");
        let hash2 = compute_content_hash(b"data2");
        assert_ne!(hash1, hash2);
    }
}
#[cfg(test)]
mod database_config_tests {
    use super::*;
    #[test]
    fn test_database_config_comprehensive() {
        use tarbox::config::DatabaseConfig;

        let configs = vec![
            DatabaseConfig {
                url: "postgresql://localhost/test".into(),
                max_connections: 1,
                min_connections: 1,
            },
            DatabaseConfig {
                url: "postgresql://user@host/db".into(),
                max_connections: 10,
                min_connections: 5,
            },
            DatabaseConfig {
                url: "postgresql://user:pass@host:5432/db?sslmode=require".into(),
                max_connections: 50,
                min_connections: 10,
            },
        ];

        for config in &configs {
            // 测试克隆
            let cloned = config.clone();
            assert_eq!(config.url, cloned.url);
            assert_eq!(config.max_connections, cloned.max_connections);
            assert_eq!(config.min_connections, cloned.min_connections);

            // 测试 Debug
            let debug = format!("{:?}", config);
            assert!(debug.contains("DatabaseConfig"));

            // 验证约束
            assert!(!config.url.is_empty());
            assert!(config.max_connections > 0);
            assert!(config.min_connections > 0);
            assert!(config.max_connections >= config.min_connections);
        }
    }
}
#[cfg(test)]
mod config_serialization_tests {
    use super::*;
    #[test]
    fn test_config_comprehensive() {
        use tarbox::config::*;

        let config = Config::default();

        // 测试所有子配置
        assert!(!config.database.url.is_empty());
        assert!(config.database.max_connections > 0);

        assert!(!config.fuse.mount_point.is_empty());

        assert!(config.audit.retention_days > 0);

        assert!(config.cache.max_entries > 0);
        assert!(config.cache.ttl_seconds > 0);

        assert!(!config.api.rest_addr.is_empty());
        assert!(!config.api.grpc_addr.is_empty());

        // 测试克隆
        let cloned = config.clone();
        assert_eq!(config.database.url, cloned.database.url);

        // 测试序列化
        let json = serde_json::to_string(&config);
        assert!(json.is_ok());

        let json_str = json.unwrap();
        assert!(json_str.contains("database"));
        assert!(json_str.contains("fuse"));
        assert!(json_str.contains("audit"));
    }
}
