use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(GnnLinkPrediction::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(GnnLinkPrediction::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .default(Expr::cust("gen_random_uuid()")),
                    )
                    .col(
                        ColumnDef::new(GnnLinkPrediction::TargetEntityId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GnnLinkPrediction::TargetEntityType)
                            .string_len(64)
                            .not_null()
                            .default("person"),
                    )
                    .col(
                        ColumnDef::new(GnnLinkPrediction::CandidateEntityId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(GnnLinkPrediction::CandidateEntityType)
                            .string_len(64)
                            .not_null()
                            .default("person"),
                    )
                    .col(
                        ColumnDef::new(GnnLinkPrediction::PredictedEdgeType)
                            .string_len(64)
                            .not_null()
                            .default("suspected_link_to"),
                    )
                    .col(ColumnDef::new(GnnLinkPrediction::LinkProbability).double().not_null())
                    .col(
                        ColumnDef::new(GnnLinkPrediction::IsHypothesisFlagged)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(GnnLinkPrediction::ConfidenceThreshold).double())
                    .col(ColumnDef::new(GnnLinkPrediction::Recommendation).text())
                    .col(
                        ColumnDef::new(GnnLinkPrediction::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(GnnLinkPrediction::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_gnn_link_prediction_target")
                    .table(GnnLinkPrediction::Table)
                    .col(GnnLinkPrediction::TargetEntityId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_gnn_link_prediction_created")
                    .table(GnnLinkPrediction::Table)
                    .col(GnnLinkPrediction::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(GnnLinkPrediction::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum GnnLinkPrediction {
    Table,
    Id,
    TargetEntityId,
    TargetEntityType,
    CandidateEntityId,
    CandidateEntityType,
    PredictedEdgeType,
    LinkProbability,
    IsHypothesisFlagged,
    ConfidenceThreshold,
    Recommendation,
    CreatedAt,
    UpdatedAt,
}
