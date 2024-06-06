use async_trait::async_trait;
use mockall::mock;
use rusoto_core::RusotoError;
use rusoto_events::*;

mock! {
    pub EBClient {}

    #[async_trait]
    impl EventBridge for EBClient {
        async fn activate_event_source(
            &self,
            input: ActivateEventSourceRequest,
        ) -> Result<(), RusotoError<ActivateEventSourceError>>;

        async fn cancel_replay(
            &self,
            input: CancelReplayRequest,
        ) -> Result<CancelReplayResponse, RusotoError<CancelReplayError>>;

        async fn create_api_destination(
            &self,
            input: CreateApiDestinationRequest,
        ) -> Result<CreateApiDestinationResponse, RusotoError<CreateApiDestinationError>>;

        async fn create_archive(
            &self,
            input: CreateArchiveRequest,
        ) -> Result<CreateArchiveResponse, RusotoError<CreateArchiveError>>;

        async fn create_connection(
            &self,
            input: CreateConnectionRequest,
        ) -> Result<CreateConnectionResponse, RusotoError<CreateConnectionError>>;

        async fn create_event_bus(
            &self,
            input: CreateEventBusRequest,
        ) -> Result<CreateEventBusResponse, RusotoError<CreateEventBusError>>;

        async fn create_partner_event_source(
            &self,
            input: CreatePartnerEventSourceRequest,
        ) -> Result<CreatePartnerEventSourceResponse, RusotoError<CreatePartnerEventSourceError>>;

        async fn deactivate_event_source(
            &self,
            input: DeactivateEventSourceRequest,
        ) -> Result<(), RusotoError<DeactivateEventSourceError>>;

        async fn deauthorize_connection(
            &self,
            input: DeauthorizeConnectionRequest,
        ) -> Result<DeauthorizeConnectionResponse, RusotoError<DeauthorizeConnectionError>>;

        async fn delete_api_destination(
            &self,
            input: DeleteApiDestinationRequest,
        ) -> Result<DeleteApiDestinationResponse, RusotoError<DeleteApiDestinationError>>;

        async fn delete_archive(
            &self,
            input: DeleteArchiveRequest,
        ) -> Result<DeleteArchiveResponse, RusotoError<DeleteArchiveError>>;

        async fn delete_connection(
            &self,
            input: DeleteConnectionRequest,
        ) -> Result<DeleteConnectionResponse, RusotoError<DeleteConnectionError>>;

        async fn delete_event_bus(
            &self,
            input: DeleteEventBusRequest,
        ) -> Result<(), RusotoError<DeleteEventBusError>>;

        async fn delete_partner_event_source(
            &self,
            input: DeletePartnerEventSourceRequest,
        ) -> Result<(), RusotoError<DeletePartnerEventSourceError>>;

        async fn delete_rule(
            &self,
            input: DeleteRuleRequest,
        ) -> Result<(), RusotoError<DeleteRuleError>>;

        async fn describe_api_destination(
            &self,
            input: DescribeApiDestinationRequest,
        ) -> Result<DescribeApiDestinationResponse, RusotoError<DescribeApiDestinationError>>;

        async fn describe_archive(
            &self,
            input: DescribeArchiveRequest,
        ) -> Result<DescribeArchiveResponse, RusotoError<DescribeArchiveError>>;

        async fn describe_connection(
            &self,
            input: DescribeConnectionRequest,
        ) -> Result<DescribeConnectionResponse, RusotoError<DescribeConnectionError>>;

        async fn describe_event_bus(
            &self,
            input: DescribeEventBusRequest,
        ) -> Result<DescribeEventBusResponse, RusotoError<DescribeEventBusError>>;

        async fn describe_event_source(
            &self,
            input: DescribeEventSourceRequest,
        ) -> Result<DescribeEventSourceResponse, RusotoError<DescribeEventSourceError>>;

        async fn describe_partner_event_source(
            &self,
            input: DescribePartnerEventSourceRequest,
        ) -> Result<DescribePartnerEventSourceResponse, RusotoError<DescribePartnerEventSourceError>>;

        async fn describe_replay(
            &self,
            input: DescribeReplayRequest,
        ) -> Result<DescribeReplayResponse, RusotoError<DescribeReplayError>>;

        async fn describe_rule(
            &self,
            input: DescribeRuleRequest,
        ) -> Result<DescribeRuleResponse, RusotoError<DescribeRuleError>>;

        async fn disable_rule(
            &self,
            input: DisableRuleRequest,
        ) -> Result<(), RusotoError<DisableRuleError>>;

        async fn enable_rule(
            &self,
            input: EnableRuleRequest,
        ) -> Result<(), RusotoError<EnableRuleError>>;

        async fn list_api_destinations(
            &self,
            input: ListApiDestinationsRequest,
        ) -> Result<ListApiDestinationsResponse, RusotoError<ListApiDestinationsError>>;

        async fn list_archives(
            &self,
            input: ListArchivesRequest,
        ) -> Result<ListArchivesResponse, RusotoError<ListArchivesError>>;

        async fn list_connections(
            &self,
            input: ListConnectionsRequest,
        ) -> Result<ListConnectionsResponse, RusotoError<ListConnectionsError>>;

        async fn list_event_buses(
            &self,
            input: ListEventBusesRequest,
        ) -> Result<ListEventBusesResponse, RusotoError<ListEventBusesError>>;

        async fn list_event_sources(
            &self,
            input: ListEventSourcesRequest,
        ) -> Result<ListEventSourcesResponse, RusotoError<ListEventSourcesError>>;

        async fn list_partner_event_source_accounts(
            &self,
            input: ListPartnerEventSourceAccountsRequest,
        ) -> Result<
            ListPartnerEventSourceAccountsResponse,
            RusotoError<ListPartnerEventSourceAccountsError>,
        >;

        async fn list_partner_event_sources(
            &self,
            input: ListPartnerEventSourcesRequest,
        ) -> Result<ListPartnerEventSourcesResponse, RusotoError<ListPartnerEventSourcesError>>;

        async fn list_replays(
            &self,
            input: ListReplaysRequest,
        ) -> Result<ListReplaysResponse, RusotoError<ListReplaysError>>;

        async fn list_rule_names_by_target(
            &self,
            input: ListRuleNamesByTargetRequest,
        ) -> Result<ListRuleNamesByTargetResponse, RusotoError<ListRuleNamesByTargetError>>;

        async fn list_rules(
            &self,
            input: ListRulesRequest,
        ) -> Result<ListRulesResponse, RusotoError<ListRulesError>>;

        async fn list_tags_for_resource(
            &self,
            input: ListTagsForResourceRequest,
        ) -> Result<ListTagsForResourceResponse, RusotoError<ListTagsForResourceError>>;

        async fn list_targets_by_rule(
            &self,
            input: ListTargetsByRuleRequest,
        ) -> Result<ListTargetsByRuleResponse, RusotoError<ListTargetsByRuleError>>;

        async fn put_events(
            &self,
            input: PutEventsRequest,
        ) -> Result<PutEventsResponse, RusotoError<PutEventsError>>;

        async fn put_partner_events(
            &self,
            input: PutPartnerEventsRequest,
        ) -> Result<PutPartnerEventsResponse, RusotoError<PutPartnerEventsError>>;

        async fn put_permission(
            &self,
            input: PutPermissionRequest,
        ) -> Result<(), RusotoError<PutPermissionError>>;

        async fn put_rule(
            &self,
            input: PutRuleRequest,
        ) -> Result<PutRuleResponse, RusotoError<PutRuleError>>;

        async fn put_targets(
            &self,
            input: PutTargetsRequest,
        ) -> Result<PutTargetsResponse, RusotoError<PutTargetsError>>;

        async fn remove_permission(
            &self,
            input: RemovePermissionRequest,
        ) -> Result<(), RusotoError<RemovePermissionError>>;

        async fn remove_targets(
            &self,
            input: RemoveTargetsRequest,
        ) -> Result<RemoveTargetsResponse, RusotoError<RemoveTargetsError>>;

        async fn start_replay(
            &self,
            input: StartReplayRequest,
        ) -> Result<StartReplayResponse, RusotoError<StartReplayError>>;

        async fn tag_resource(
            &self,
            input: TagResourceRequest,
        ) -> Result<TagResourceResponse, RusotoError<TagResourceError>>;

        async fn test_event_pattern(
            &self,
            input: TestEventPatternRequest,
        ) -> Result<TestEventPatternResponse, RusotoError<TestEventPatternError>>;

        async fn untag_resource(
            &self,
            input: UntagResourceRequest,
        ) -> Result<UntagResourceResponse, RusotoError<UntagResourceError>>;

        async fn update_api_destination(
            &self,
            input: UpdateApiDestinationRequest,
        ) -> Result<UpdateApiDestinationResponse, RusotoError<UpdateApiDestinationError>>;

        async fn update_archive(
            &self,
            input: UpdateArchiveRequest,
        ) -> Result<UpdateArchiveResponse, RusotoError<UpdateArchiveError>>;

        async fn update_connection(
            &self,
            input: UpdateConnectionRequest,
        ) -> Result<UpdateConnectionResponse, RusotoError<UpdateConnectionError>>;
    }
}
