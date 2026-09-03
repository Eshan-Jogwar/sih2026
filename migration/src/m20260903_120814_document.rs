use sea_orm_migration::{
    prelude::*,
    sea_query::{ForeignKeyAction::Cascade, extension::postgres::Type},
};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20260903_120814_document"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // --- case ---
        manager
            .create_table(
                Table::create()
                    .table(Case::Table)
                    .col(
                        ColumnDef::new(Case::Id)
                            .uuid()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(Case::Name).string().not_null())
                    .col(
                        ColumnDef::new(Case::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Case::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // --- document ---
        manager
            .create_type(
                Type::create()
                    .as_enum(DocumentStatus::Type)
                    .values([
                        DocumentStatus::Pending,
                        DocumentStatus::Processing,
                        DocumentStatus::Success,
                        DocumentStatus::Failed,
                        DocumentStatus::Finish,
                    ])
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Document::Table)
                    .col(
                        ColumnDef::new(Document::Id)
                            .uuid()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(ColumnDef::new(Document::Title).string().not_null())
                    .col(ColumnDef::new(Document::Description).string().not_null())
                    .col(
                        ColumnDef::new(Document::Status)
                            .custom(DocumentStatus::Type)
                            .not_null()
                            .default("pending"),
                    )
                    .col(ColumnDef::new(Document::ObjectKey).string().not_null())
                    .col(ColumnDef::new(Document::ExtractedInformation).json())
                    .col(ColumnDef::new(Document::CaseId).uuid().not_null())
                    .col(
                        ColumnDef::new(Document::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Document::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-document-case_id")
                            .from(Document::Table, Document::CaseId)
                            .to(Case::Table, Case::Id)
                            .on_delete(Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-document-case_id")
                    .table(Document::Table)
                    .col(Document::CaseId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // reverse order: document depends on case, so drop document first
        manager
            .drop_table(Table::drop().table(Document::Table).to_owned())
            .await?;
        manager
            .drop_type(Type::drop().name(DocumentStatus::Type).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Case::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum DocumentStatus {
    #[sea_orm(iden = "document_status")]
    Type,
    #[sea_orm(iden = "pending")]
    Pending,
    #[sea_orm(iden = "processing")]
    Processing,
    #[sea_orm(iden = "success")]
    Success,
    #[sea_orm(iden = "failed")]
    Failed,
    #[sea_orm(iden = "finish")]
    Finish,
}

#[derive(DeriveIden)]
pub enum Case {
    Table,
    Id,
    Name,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
pub enum Document {
    Table,
    Id,
    Title,
    Description,
    Status,
    ObjectKey,
    ExtractedInformation,
    CaseId,
    CreatedAt,
    UpdatedAt,
}
