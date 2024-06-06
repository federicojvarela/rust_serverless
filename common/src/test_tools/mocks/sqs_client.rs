use async_trait::async_trait;
use mockall::mock;
use rusoto_core::RusotoError;
use rusoto_sqs::*;

mock! {
    pub SqsClient {}
    #[async_trait]
    impl Sqs for SqsClient {
        async fn add_permission(
            &self,
            input: AddPermissionRequest,
        ) -> Result<(), RusotoError<AddPermissionError>>;

        async fn change_message_visibility(
            &self,
            input: ChangeMessageVisibilityRequest,
        ) -> Result<(), RusotoError<ChangeMessageVisibilityError>>;

        async fn change_message_visibility_batch(
            &self,
            input: ChangeMessageVisibilityBatchRequest,
        ) -> Result<ChangeMessageVisibilityBatchResult, RusotoError<ChangeMessageVisibilityBatchError>>;

        async fn create_queue(
            &self,
            input: CreateQueueRequest,
        ) -> Result<CreateQueueResult, RusotoError<CreateQueueError>>;

        async fn delete_message(
            &self,
            input: DeleteMessageRequest,
        ) -> Result<(), RusotoError<DeleteMessageError>>;

        async fn delete_message_batch(
            &self,
            input: DeleteMessageBatchRequest,
        ) -> Result<DeleteMessageBatchResult, RusotoError<DeleteMessageBatchError>>;

        async fn delete_queue(
            &self,
            input: DeleteQueueRequest,
        ) -> Result<(), RusotoError<DeleteQueueError>>;

        async fn get_queue_attributes(
            &self,
            input: GetQueueAttributesRequest,
        ) -> Result<GetQueueAttributesResult, RusotoError<GetQueueAttributesError>>;

        async fn get_queue_url(
            &self,
            input: GetQueueUrlRequest,
        ) -> Result<GetQueueUrlResult, RusotoError<GetQueueUrlError>>;

        async fn list_dead_letter_source_queues(
            &self,
            input: ListDeadLetterSourceQueuesRequest,
        ) -> Result<ListDeadLetterSourceQueuesResult, RusotoError<ListDeadLetterSourceQueuesError>>;

        async fn list_queue_tags(
            &self,
            input: ListQueueTagsRequest,
        ) -> Result<ListQueueTagsResult, RusotoError<ListQueueTagsError>>;

        async fn list_queues(
            &self,
            input: ListQueuesRequest,
        ) -> Result<ListQueuesResult, RusotoError<ListQueuesError>>;

        async fn purge_queue(
            &self,
            input: PurgeQueueRequest,
        ) -> Result<(), RusotoError<PurgeQueueError>>;

        async fn receive_message(
            &self,
            input: ReceiveMessageRequest,
        ) -> Result<ReceiveMessageResult, RusotoError<ReceiveMessageError>>;

        async fn remove_permission(
            &self,
            input: RemovePermissionRequest,
        ) -> Result<(), RusotoError<RemovePermissionError>>;

        async fn send_message(
            &self,
            input: SendMessageRequest,
        ) -> Result<SendMessageResult, RusotoError<SendMessageError>>;

        async fn send_message_batch(
            &self,
            input: SendMessageBatchRequest,
        ) -> Result<SendMessageBatchResult, RusotoError<SendMessageBatchError>>;

        async fn set_queue_attributes(
            &self,
            input: SetQueueAttributesRequest,
        ) -> Result<(), RusotoError<SetQueueAttributesError>>;

        async fn tag_queue(&self, input: TagQueueRequest) -> Result<(), RusotoError<TagQueueError>>;

        async fn untag_queue(
            &self,
            input: UntagQueueRequest,
        ) -> Result<(), RusotoError<UntagQueueError>>;
    }
}
