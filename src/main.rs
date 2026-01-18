use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tarbox::config::DatabaseConfig;
use tarbox::fs::FileSystem;
use tarbox::fuse::{MountOptions, mount, unmount};
use tarbox::storage::{
    CreateTenantInput, DatabasePool, InodeType, TenantOperations, TenantRepository,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "tarbox")]
#[command(about = "PostgreSQL-based filesystem for AI agents", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(long, global = true, help = "Tenant name (required for file operations)")]
    tenant: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Initialize database schema")]
    Init,

    #[command(subcommand, about = "Tenant management commands")]
    Tenant(TenantCommands),

    #[command(about = "Create directory")]
    Mkdir {
        #[arg(help = "Directory path to create")]
        path: String,
    },

    #[command(about = "List directory contents")]
    Ls {
        #[arg(default_value = "/", help = "Directory path to list")]
        path: String,
    },

    #[command(about = "Remove empty directory")]
    Rmdir {
        #[arg(help = "Directory path to remove")]
        path: String,
    },

    #[command(about = "Create empty file")]
    Touch {
        #[arg(help = "File path to create")]
        path: String,
    },

    #[command(about = "Write content to file")]
    Write {
        #[arg(help = "File path")]
        path: String,
        #[arg(help = "Content to write")]
        content: String,
    },

    #[command(about = "Read and display file content")]
    Cat {
        #[arg(help = "File path to read")]
        path: String,
    },

    #[command(about = "Remove file")]
    Rm {
        #[arg(help = "File path to remove")]
        path: String,
    },

    #[command(about = "Display file or directory information")]
    Stat {
        #[arg(help = "Path to stat")]
        path: String,
    },

    #[command(about = "Mount filesystem via FUSE")]
    Mount {
        #[arg(help = "Mount point directory")]
        mountpoint: String,

        #[arg(long, help = "Allow other users to access")]
        allow_other: bool,

        #[arg(long, help = "Allow root to access")]
        allow_root: bool,

        #[arg(long, help = "Mount as read-only")]
        read_only: bool,
    },

    #[command(about = "Unmount FUSE filesystem")]
    Umount {
        #[arg(help = "Mount point directory")]
        mountpoint: String,
    },
}

#[derive(Subcommand)]
enum TenantCommands {
    #[command(about = "Create new tenant")]
    Create {
        #[arg(help = "Tenant name")]
        name: String,
    },

    #[command(about = "Display tenant information")]
    Info {
        #[arg(help = "Tenant name")]
        name: String,
    },

    #[command(about = "List all tenants")]
    List,

    #[command(about = "Delete tenant and all its data")]
    Delete {
        #[arg(help = "Tenant name")]
        name: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tarbox=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    let config = DatabaseConfig {
        url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/tarbox".into()),
        max_connections: 10,
        min_connections: 2,
    };

    match cli.command {
        Commands::Init => {
            let pool = DatabasePool::new(&config).await?;
            pool.run_migrations().await?;
            println!("Database schema initialized successfully");
            Ok(())
        }
        Commands::Tenant(tenant_cmd) => {
            let pool = DatabasePool::new(&config).await?;
            let tenant_ops = TenantOperations::new(pool.pool());
            handle_tenant_command(tenant_cmd, tenant_ops).await
        }
        Commands::Mkdir { path } => {
            let tenant_id = get_tenant_id(&config, &cli.tenant).await?;
            let pool = DatabasePool::new(&config).await?;
            let fs = FileSystem::new(pool.pool(), tenant_id).await?;
            fs.create_directory(&path).await?;
            println!("Created directory: {}", path);
            Ok(())
        }
        Commands::Ls { path } => {
            let tenant_id = get_tenant_id(&config, &cli.tenant).await?;
            let pool = DatabasePool::new(&config).await?;
            let fs = FileSystem::new(pool.pool(), tenant_id).await?;
            let entries = fs.list_directory(&path).await?;
            for entry in entries {
                let suffix = if entry.inode_type == InodeType::Dir { "/" } else { "" };
                println!("{}{}", entry.name, suffix);
            }
            Ok(())
        }
        Commands::Rmdir { path } => {
            let tenant_id = get_tenant_id(&config, &cli.tenant).await?;
            let pool = DatabasePool::new(&config).await?;
            let fs = FileSystem::new(pool.pool(), tenant_id).await?;
            fs.remove_directory(&path).await?;
            println!("Removed directory: {}", path);
            Ok(())
        }
        Commands::Touch { path } => {
            let tenant_id = get_tenant_id(&config, &cli.tenant).await?;
            let pool = DatabasePool::new(&config).await?;
            let fs = FileSystem::new(pool.pool(), tenant_id).await?;
            fs.create_file(&path).await?;
            println!("Created file: {}", path);
            Ok(())
        }
        Commands::Write { path, content } => {
            let tenant_id = get_tenant_id(&config, &cli.tenant).await?;
            let pool = DatabasePool::new(&config).await?;
            let fs = FileSystem::new(pool.pool(), tenant_id).await?;
            fs.write_file(&path, content.as_bytes()).await?;
            println!("Wrote {} bytes to {}", content.len(), path);
            Ok(())
        }
        Commands::Cat { path } => {
            let tenant_id = get_tenant_id(&config, &cli.tenant).await?;
            let pool = DatabasePool::new(&config).await?;
            let fs = FileSystem::new(pool.pool(), tenant_id).await?;
            let data = fs.read_file(&path).await?;
            let content = String::from_utf8_lossy(&data);
            print!("{}", content);
            Ok(())
        }
        Commands::Rm { path } => {
            let tenant_id = get_tenant_id(&config, &cli.tenant).await?;
            let pool = DatabasePool::new(&config).await?;
            let fs = FileSystem::new(pool.pool(), tenant_id).await?;
            fs.delete_file(&path).await?;
            println!("Removed file: {}", path);
            Ok(())
        }
        Commands::Stat { path } => {
            let tenant_id = get_tenant_id(&config, &cli.tenant).await?;
            let pool = DatabasePool::new(&config).await?;
            let fs = FileSystem::new(pool.pool(), tenant_id).await?;
            let inode = fs.stat(&path).await?;
            println!("  File: {}", path);
            println!("  Size: {}", inode.size);
            println!("  Type: {:?}", inode.inode_type);
            println!("  Mode: {:o}", inode.mode);
            println!("   Uid: {}", inode.uid);
            println!("   Gid: {}", inode.gid);
            println!("Access: {}", inode.atime);
            println!("Modify: {}", inode.mtime);
            println!("Change: {}", inode.ctime);
            Ok(())
        }
        Commands::Mount { mountpoint, allow_other, allow_root, read_only } => {
            let tenant_id = get_tenant_id(&config, &cli.tenant).await?;
            let pool = DatabasePool::new(&config).await?;

            let mount_options = MountOptions {
                allow_other,
                allow_root,
                read_only,
                fsname: Some(format!("tarbox:{}", cli.tenant.as_ref().unwrap())),
                auto_unmount: true,
            };

            println!("Mounting Tarbox filesystem at: {}", mountpoint);
            println!("Tenant: {}", cli.tenant.as_ref().unwrap());
            println!("Press Ctrl+C to unmount");

            let backend = Arc::new(
                tarbox::fuse::backend::TarboxBackend::new(Arc::new(pool.pool().clone()), tenant_id)
                    .await?,
            );
            let _session = mount(backend, &mountpoint, mount_options)?;

            // Keep the process running until Ctrl+C
            tokio::signal::ctrl_c().await?;

            println!("\nUnmounting filesystem...");
            Ok(())
        }
        Commands::Umount { mountpoint } => {
            unmount(&mountpoint)?;
            println!("Unmounted: {}", mountpoint);
            Ok(())
        }
    }
}

async fn handle_tenant_command(
    command: TenantCommands,
    tenant_ops: TenantOperations<'_>,
) -> Result<()> {
    match command {
        TenantCommands::Create { name } => {
            let tenant = tenant_ops.create(CreateTenantInput { tenant_name: name.clone() }).await?;
            println!("Created tenant: {}", name);
            println!("Tenant ID: {}", tenant.tenant_id);
            println!("Root inode: {}", tenant.root_inode_id);
            Ok(())
        }
        TenantCommands::Info { name } => {
            let tenant = tenant_ops.get_by_name(&name).await?;
            match tenant {
                Some(t) => {
                    println!("Tenant: {}", t.tenant_name);
                    println!("  ID: {}", t.tenant_id);
                    println!("  Root inode: {}", t.root_inode_id);
                    println!("  Created: {}", t.created_at);
                    Ok(())
                }
                None => {
                    eprintln!("Tenant not found: {}", name);
                    std::process::exit(1);
                }
            }
        }
        TenantCommands::List => {
            let tenants = tenant_ops.list().await?;
            if tenants.is_empty() {
                println!("No tenants found");
            } else {
                println!("Tenants:");
                for tenant in tenants {
                    println!("  {} ({})", tenant.tenant_name, tenant.tenant_id);
                }
            }
            Ok(())
        }
        TenantCommands::Delete { name } => {
            let tenant = tenant_ops.get_by_name(&name).await?;
            match tenant {
                Some(t) => {
                    tenant_ops.delete(t.tenant_id).await?;
                    println!("Deleted tenant: {}", name);
                    Ok(())
                }
                None => {
                    eprintln!("Tenant not found: {}", name);
                    std::process::exit(1);
                }
            }
        }
    }
}

async fn get_tenant_id(config: &DatabaseConfig, tenant_name: &Option<String>) -> Result<Uuid> {
    let name = tenant_name
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("--tenant option is required for file operations"))?;

    let pool = DatabasePool::new(config).await?;
    let tenant_ops = TenantOperations::new(pool.pool());

    let tenant = tenant_ops
        .get_by_name(name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Tenant not found: {}", name))?;

    Ok(tenant.tenant_id)
}
