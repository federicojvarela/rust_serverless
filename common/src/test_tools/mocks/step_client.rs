use async_trait::async_trait;
use mockall::mock;
use rusoto_core::RusotoError;
use rusoto_stepfunctions::*;

mock! {
    pub StepsClient {}
    #[async_trait]
    impl StepFunctions for StepsClient {
        async fn create_activity(
            &self,
            input: CreateActivityInput,
        ) -> Result<CreateActivityOutput, RusotoError<CreateActivityError>>;
        async fn create_state_machine(
            &self,
            input: CreateStateMachineInput,
        ) -> Result<CreateStateMachineOutput, RusotoError<CreateStateMachineError>>;
        async fn delete_activity(
            &self,
            input: DeleteActivityInput,
        ) -> Result<DeleteActivityOutput, RusotoError<DeleteActivityError>>;
        async fn delete_state_machine(
            &self,
            input: DeleteStateMachineInput,
        ) -> Result<DeleteStateMachineOutput, RusotoError<DeleteStateMachineError>>;
        async fn describe_activity(
            &self,
            input: DescribeActivityInput,
        ) -> Result<DescribeActivityOutput, RusotoError<DescribeActivityError>>;
        async fn describe_execution(
            &self,
            input: DescribeExecutionInput,
        ) -> Result<DescribeExecutionOutput, RusotoError<DescribeExecutionError>>;
        async fn describe_state_machine(
            &self,
            input: DescribeStateMachineInput,
        ) -> Result<DescribeStateMachineOutput, RusotoError<DescribeStateMachineError>>;
        async fn describe_state_machine_for_execution(
            &self,
            input: DescribeStateMachineForExecutionInput,
        ) -> Result<
            DescribeStateMachineForExecutionOutput,
            RusotoError<DescribeStateMachineForExecutionError>,
        >;
        async fn get_activity_task(
            &self,
            input: GetActivityTaskInput,
        ) -> Result<GetActivityTaskOutput, RusotoError<GetActivityTaskError>>;
        async fn get_execution_history(
            &self,
            input: GetExecutionHistoryInput,
        ) -> Result<GetExecutionHistoryOutput, RusotoError<GetExecutionHistoryError>>;
        async fn list_activities(
            &self,
            input: ListActivitiesInput,
        ) -> Result<ListActivitiesOutput, RusotoError<ListActivitiesError>>;
        async fn list_executions(
            &self,
            input: ListExecutionsInput,
        ) -> Result<ListExecutionsOutput, RusotoError<ListExecutionsError>>;
        async fn list_state_machines(
            &self,
            input: ListStateMachinesInput,
        ) -> Result<ListStateMachinesOutput, RusotoError<ListStateMachinesError>>;
        async fn list_tags_for_resource(
            &self,
            input: ListTagsForResourceInput,
        ) -> Result<ListTagsForResourceOutput, RusotoError<ListTagsForResourceError>>;
        async fn send_task_failure(
            &self,
            input: SendTaskFailureInput,
        ) -> Result<SendTaskFailureOutput, RusotoError<SendTaskFailureError>>;
        async fn send_task_heartbeat(
            &self,
            input: SendTaskHeartbeatInput,
        ) -> Result<SendTaskHeartbeatOutput, RusotoError<SendTaskHeartbeatError>>;
        async fn send_task_success(
            &self,
            input: SendTaskSuccessInput,
        ) -> Result<SendTaskSuccessOutput, RusotoError<SendTaskSuccessError>>;
        async fn start_execution(
            &self,
            input: StartExecutionInput,
        ) -> Result<StartExecutionOutput, RusotoError<StartExecutionError>>;
        async fn start_sync_execution(
            &self,
            input: StartSyncExecutionInput,
        ) -> Result<StartSyncExecutionOutput, RusotoError<StartSyncExecutionError>>;
        async fn stop_execution(
            &self,
            input: StopExecutionInput,
        ) -> Result<StopExecutionOutput, RusotoError<StopExecutionError>>;
        async fn tag_resource(
            &self,
            input: TagResourceInput,
        ) -> Result<TagResourceOutput, RusotoError<TagResourceError>>;
        async fn untag_resource(
            &self,
            input: UntagResourceInput,
        ) -> Result<UntagResourceOutput, RusotoError<UntagResourceError>>;
        async fn update_state_machine(
            &self,
            input: UpdateStateMachineInput,
        ) -> Result<UpdateStateMachineOutput, RusotoError<UpdateStateMachineError>>;
    }
}
