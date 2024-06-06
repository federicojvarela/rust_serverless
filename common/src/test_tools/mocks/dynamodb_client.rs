use async_trait::async_trait;
use mockall::mock;
use rusoto_core::RusotoError;
use rusoto_dynamodb::*;

mock! {
    pub DbClient {}

    #[async_trait]
    impl DynamoDb for DbClient {
        async fn batch_execute_statement(
            &self,
            input: BatchExecuteStatementInput,
        ) -> Result<BatchExecuteStatementOutput, RusotoError<BatchExecuteStatementError>>;

        async fn batch_get_item(
            &self,
            input: BatchGetItemInput,
        ) -> Result<BatchGetItemOutput, RusotoError<BatchGetItemError>>;

        async fn batch_write_item(
            &self,
            input: BatchWriteItemInput,
        ) -> Result<BatchWriteItemOutput, RusotoError<BatchWriteItemError>>;

        async fn create_backup(
            &self,
            input: CreateBackupInput,
        ) -> Result<CreateBackupOutput, RusotoError<CreateBackupError>>;

        async fn create_global_table(
            &self,
            input: CreateGlobalTableInput,
        ) -> Result<CreateGlobalTableOutput, RusotoError<CreateGlobalTableError>>;

        async fn create_table(
            &self,
            input: CreateTableInput,
        ) -> Result<CreateTableOutput, RusotoError<CreateTableError>>;

        async fn delete_backup(
            &self,
            input: DeleteBackupInput,
        ) -> Result<DeleteBackupOutput, RusotoError<DeleteBackupError>>;

        async fn delete_item(
            &self,
            input: DeleteItemInput,
        ) -> Result<DeleteItemOutput, RusotoError<DeleteItemError>>;

        async fn delete_table(
            &self,
            input: DeleteTableInput,
        ) -> Result<DeleteTableOutput, RusotoError<DeleteTableError>>;

        async fn describe_backup(
            &self,
            input: DescribeBackupInput,
        ) -> Result<DescribeBackupOutput, RusotoError<DescribeBackupError>>;

        async fn describe_continuous_backups(
            &self,
            input: DescribeContinuousBackupsInput,
        ) -> Result<DescribeContinuousBackupsOutput, RusotoError<DescribeContinuousBackupsError>>;

        async fn describe_contributor_insights(
            &self,
            input: DescribeContributorInsightsInput,
        ) -> Result<DescribeContributorInsightsOutput, RusotoError<DescribeContributorInsightsError>>;

        async fn describe_endpoints(
            &self,
        ) -> Result<DescribeEndpointsResponse, RusotoError<DescribeEndpointsError>>;

        async fn describe_export(
            &self,
            input: DescribeExportInput,
        ) -> Result<DescribeExportOutput, RusotoError<DescribeExportError>>;

        async fn describe_global_table(
            &self,
            input: DescribeGlobalTableInput,
        ) -> Result<DescribeGlobalTableOutput, RusotoError<DescribeGlobalTableError>>;

        async fn describe_global_table_settings(
            &self,
            input: DescribeGlobalTableSettingsInput,
        ) -> Result<DescribeGlobalTableSettingsOutput, RusotoError<DescribeGlobalTableSettingsError>>;

        async fn describe_kinesis_streaming_destination(
            &self,
            input: DescribeKinesisStreamingDestinationInput,
        ) -> Result<
            DescribeKinesisStreamingDestinationOutput,
            RusotoError<DescribeKinesisStreamingDestinationError>,
        >;

        async fn describe_limits(
            &self,
        ) -> Result<DescribeLimitsOutput, RusotoError<DescribeLimitsError>>;

        async fn describe_table(
            &self,
            input: DescribeTableInput,
        ) -> Result<DescribeTableOutput, RusotoError<DescribeTableError>>;

        async fn describe_table_replica_auto_scaling(
            &self,
            input: DescribeTableReplicaAutoScalingInput,
        ) -> Result<
            DescribeTableReplicaAutoScalingOutput,
            RusotoError<DescribeTableReplicaAutoScalingError>,
        >;

        async fn describe_time_to_live(
            &self,
            input: DescribeTimeToLiveInput,
        ) -> Result<DescribeTimeToLiveOutput, RusotoError<DescribeTimeToLiveError>>;

        async fn disable_kinesis_streaming_destination(
            &self,
            input: KinesisStreamingDestinationInput,
        ) -> Result<
            KinesisStreamingDestinationOutput,
            RusotoError<DisableKinesisStreamingDestinationError>,
        >;

        async fn enable_kinesis_streaming_destination(
            &self,
            input: KinesisStreamingDestinationInput,
        ) -> Result<
            KinesisStreamingDestinationOutput,
            RusotoError<EnableKinesisStreamingDestinationError>,
        >;

        async fn execute_statement(
            &self,
            input: ExecuteStatementInput,
        ) -> Result<ExecuteStatementOutput, RusotoError<ExecuteStatementError>>;

        async fn execute_transaction(
            &self,
            input: ExecuteTransactionInput,
        ) -> Result<ExecuteTransactionOutput, RusotoError<ExecuteTransactionError>>;

        async fn export_table_to_point_in_time(
            &self,
            input: ExportTableToPointInTimeInput,
        ) -> Result<ExportTableToPointInTimeOutput, RusotoError<ExportTableToPointInTimeError>>;

        async fn get_item(
            &self,
            input: GetItemInput,
        ) -> Result<GetItemOutput, RusotoError<GetItemError>>;

        async fn list_backups(
            &self,
            input: ListBackupsInput,
        ) -> Result<ListBackupsOutput, RusotoError<ListBackupsError>>;

        async fn list_contributor_insights(
            &self,
            input: ListContributorInsightsInput,
        ) -> Result<ListContributorInsightsOutput, RusotoError<ListContributorInsightsError>>;

        async fn list_exports(
            &self,
            input: ListExportsInput,
        ) -> Result<ListExportsOutput, RusotoError<ListExportsError>>;

        async fn list_global_tables(
            &self,
            input: ListGlobalTablesInput,
        ) -> Result<ListGlobalTablesOutput, RusotoError<ListGlobalTablesError>>;

        async fn list_tables(
            &self,
            input: ListTablesInput,
        ) -> Result<ListTablesOutput, RusotoError<ListTablesError>>;

        async fn list_tags_of_resource(
            &self,
            input: ListTagsOfResourceInput,
        ) -> Result<ListTagsOfResourceOutput, RusotoError<ListTagsOfResourceError>>;

        async fn put_item(
            &self,
            input: PutItemInput,
        ) -> Result<PutItemOutput, RusotoError<PutItemError>>;

        async fn query(&self, input: QueryInput) -> Result<QueryOutput, RusotoError<QueryError>>;

        async fn restore_table_from_backup(
            &self,
            input: RestoreTableFromBackupInput,
        ) -> Result<RestoreTableFromBackupOutput, RusotoError<RestoreTableFromBackupError>>;

        async fn restore_table_to_point_in_time(
            &self,
            input: RestoreTableToPointInTimeInput,
        ) -> Result<RestoreTableToPointInTimeOutput, RusotoError<RestoreTableToPointInTimeError>>;

        async fn scan(&self, input: ScanInput) -> Result<ScanOutput, RusotoError<ScanError>>;

        async fn tag_resource(
            &self,
            input: TagResourceInput,
        ) -> Result<(), RusotoError<TagResourceError>>;

        async fn transact_get_items(
            &self,
            input: TransactGetItemsInput,
        ) -> Result<TransactGetItemsOutput, RusotoError<TransactGetItemsError>>;

        async fn transact_write_items(
            &self,
            input: TransactWriteItemsInput,
        ) -> Result<TransactWriteItemsOutput, RusotoError<TransactWriteItemsError>>;

        async fn untag_resource(
            &self,
            input: UntagResourceInput,
        ) -> Result<(), RusotoError<UntagResourceError>>;

        async fn update_continuous_backups(
            &self,
            input: UpdateContinuousBackupsInput,
        ) -> Result<UpdateContinuousBackupsOutput, RusotoError<UpdateContinuousBackupsError>>;

        async fn update_contributor_insights(
            &self,
            input: UpdateContributorInsightsInput,
        ) -> Result<UpdateContributorInsightsOutput, RusotoError<UpdateContributorInsightsError>>;

        async fn update_global_table(
            &self,
            input: UpdateGlobalTableInput,
        ) -> Result<UpdateGlobalTableOutput, RusotoError<UpdateGlobalTableError>>;

        async fn update_global_table_settings(
            &self,
            input: UpdateGlobalTableSettingsInput,
        ) -> Result<UpdateGlobalTableSettingsOutput, RusotoError<UpdateGlobalTableSettingsError>>;

        async fn update_item(
            &self,
            input: UpdateItemInput,
        ) -> Result<UpdateItemOutput, RusotoError<UpdateItemError>>;

        async fn update_table(
            &self,
            input: UpdateTableInput,
        ) -> Result<UpdateTableOutput, RusotoError<UpdateTableError>>;

        async fn update_table_replica_auto_scaling(
            &self,
            input: UpdateTableReplicaAutoScalingInput,
        ) -> Result<UpdateTableReplicaAutoScalingOutput, RusotoError<UpdateTableReplicaAutoScalingError>>;

        async fn update_time_to_live(
            &self,
            input: UpdateTimeToLiveInput,
        ) -> Result<UpdateTimeToLiveOutput, RusotoError<UpdateTimeToLiveError>>;
    }
}
