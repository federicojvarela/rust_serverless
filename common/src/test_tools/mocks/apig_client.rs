use async_trait::async_trait;
use mockall::mock;
use rusoto_apigateway::*;
use rusoto_core::RusotoError;

mock! {
    pub ApiGClient {}
    #[async_trait]
    impl ApiGateway for ApiGClient {
        async fn create_api_key(
            &self,
            input: CreateApiKeyRequest,
        ) -> Result<ApiKey, RusotoError<CreateApiKeyError>>;

        async fn create_authorizer(
            &self,
            input: CreateAuthorizerRequest,
        ) -> Result<Authorizer, RusotoError<CreateAuthorizerError>>;

        async fn create_base_path_mapping(
            &self,
            input: CreateBasePathMappingRequest,
        ) -> Result<BasePathMapping, RusotoError<CreateBasePathMappingError>>;

        async fn create_deployment(
            &self,
            input: CreateDeploymentRequest,
        ) -> Result<Deployment, RusotoError<CreateDeploymentError>>;

        async fn create_documentation_part(
            &self,
            input: CreateDocumentationPartRequest,
        ) -> Result<DocumentationPart, RusotoError<CreateDocumentationPartError>>;

        async fn create_documentation_version(
            &self,
            input: CreateDocumentationVersionRequest,
        ) -> Result<DocumentationVersion, RusotoError<CreateDocumentationVersionError>>;

        async fn create_domain_name(
            &self,
            input: CreateDomainNameRequest,
        ) -> Result<DomainName, RusotoError<CreateDomainNameError>>;

        async fn create_model(
            &self,
            input: CreateModelRequest,
        ) -> Result<Model, RusotoError<CreateModelError>>;

        async fn create_request_validator(
            &self,
            input: CreateRequestValidatorRequest,
        ) -> Result<RequestValidator, RusotoError<CreateRequestValidatorError>>;

        async fn create_resource(
            &self,
            input: CreateResourceRequest,
        ) -> Result<Resource, RusotoError<CreateResourceError>>;

        async fn create_rest_api(
            &self,
            input: CreateRestApiRequest,
        ) -> Result<RestApi, RusotoError<CreateRestApiError>>;

        async fn create_stage(
            &self,
            input: CreateStageRequest,
        ) -> Result<Stage, RusotoError<CreateStageError>>;

        async fn create_usage_plan(
            &self,
            input: CreateUsagePlanRequest,
        ) -> Result<UsagePlan, RusotoError<CreateUsagePlanError>>;

        async fn create_usage_plan_key(
            &self,
            input: CreateUsagePlanKeyRequest,
        ) -> Result<UsagePlanKey, RusotoError<CreateUsagePlanKeyError>>;

        async fn create_vpc_link(
            &self,
            input: CreateVpcLinkRequest,
        ) -> Result<VpcLink, RusotoError<CreateVpcLinkError>>;

        async fn delete_api_key(
            &self,
            input: DeleteApiKeyRequest,
        ) -> Result<(), RusotoError<DeleteApiKeyError>>;

        async fn delete_authorizer(
            &self,
            input: DeleteAuthorizerRequest,
        ) -> Result<(), RusotoError<DeleteAuthorizerError>>;

        async fn delete_base_path_mapping(
            &self,
            input: DeleteBasePathMappingRequest,
        ) -> Result<(), RusotoError<DeleteBasePathMappingError>>;

        async fn delete_client_certificate(
            &self,
            input: DeleteClientCertificateRequest,
        ) -> Result<(), RusotoError<DeleteClientCertificateError>>;

        async fn delete_deployment(
            &self,
            input: DeleteDeploymentRequest,
        ) -> Result<(), RusotoError<DeleteDeploymentError>>;

        async fn delete_documentation_part(
            &self,
            input: DeleteDocumentationPartRequest,
        ) -> Result<(), RusotoError<DeleteDocumentationPartError>>;

        async fn delete_documentation_version(
            &self,
            input: DeleteDocumentationVersionRequest,
        ) -> Result<(), RusotoError<DeleteDocumentationVersionError>>;

        async fn delete_domain_name(
            &self,
            input: DeleteDomainNameRequest,
        ) -> Result<(), RusotoError<DeleteDomainNameError>>;

        async fn delete_gateway_response(
            &self,
            input: DeleteGatewayResponseRequest,
        ) -> Result<(), RusotoError<DeleteGatewayResponseError>>;

        async fn delete_integration(
            &self,
            input: DeleteIntegrationRequest,
        ) -> Result<(), RusotoError<DeleteIntegrationError>>;

        async fn delete_integration_response(
            &self,
            input: DeleteIntegrationResponseRequest,
        ) -> Result<(), RusotoError<DeleteIntegrationResponseError>>;

        async fn delete_method(
            &self,
            input: DeleteMethodRequest,
        ) -> Result<(), RusotoError<DeleteMethodError>>;

        async fn delete_method_response(
            &self,
            input: DeleteMethodResponseRequest,
        ) -> Result<(), RusotoError<DeleteMethodResponseError>>;

        async fn delete_model(
            &self,
            input: DeleteModelRequest,
        ) -> Result<(), RusotoError<DeleteModelError>>;

        async fn delete_request_validator(
            &self,
            input: DeleteRequestValidatorRequest,
        ) -> Result<(), RusotoError<DeleteRequestValidatorError>>;

        async fn delete_resource(
            &self,
            input: DeleteResourceRequest,
        ) -> Result<(), RusotoError<DeleteResourceError>>;

        async fn delete_rest_api(
            &self,
            input: DeleteRestApiRequest,
        ) -> Result<(), RusotoError<DeleteRestApiError>>;

        async fn delete_stage(
            &self,
            input: DeleteStageRequest,
        ) -> Result<(), RusotoError<DeleteStageError>>;

        async fn delete_usage_plan(
            &self,
            input: DeleteUsagePlanRequest,
        ) -> Result<(), RusotoError<DeleteUsagePlanError>>;

        async fn delete_usage_plan_key(
            &self,
            input: DeleteUsagePlanKeyRequest,
        ) -> Result<(), RusotoError<DeleteUsagePlanKeyError>>;

        async fn delete_vpc_link(
            &self,
            input: DeleteVpcLinkRequest,
        ) -> Result<(), RusotoError<DeleteVpcLinkError>>;

        async fn flush_stage_authorizers_cache(
            &self,
            input: FlushStageAuthorizersCacheRequest,
        ) -> Result<(), RusotoError<FlushStageAuthorizersCacheError>>;

        async fn flush_stage_cache(
            &self,
            input: FlushStageCacheRequest,
        ) -> Result<(), RusotoError<FlushStageCacheError>>;

        async fn generate_client_certificate(
            &self,
            input: GenerateClientCertificateRequest,
        ) -> Result<ClientCertificate, RusotoError<GenerateClientCertificateError>>;

        async fn get_account(&self) -> Result<Account, RusotoError<GetAccountError>>;

        async fn get_api_key(
            &self,
            input: GetApiKeyRequest,
        ) -> Result<ApiKey, RusotoError<GetApiKeyError>>;

        async fn get_api_keys(
            &self,
            input: GetApiKeysRequest,
        ) -> Result<ApiKeys, RusotoError<GetApiKeysError>>;

        async fn get_authorizer(
            &self,
            input: GetAuthorizerRequest,
        ) -> Result<Authorizer, RusotoError<GetAuthorizerError>>;

        async fn get_authorizers(
            &self,
            input: GetAuthorizersRequest,
        ) -> Result<Authorizers, RusotoError<GetAuthorizersError>>;

        async fn get_base_path_mapping(
            &self,
            input: GetBasePathMappingRequest,
        ) -> Result<BasePathMapping, RusotoError<GetBasePathMappingError>>;

        async fn get_base_path_mappings(
            &self,
            input: GetBasePathMappingsRequest,
        ) -> Result<BasePathMappings, RusotoError<GetBasePathMappingsError>>;

        async fn get_client_certificate(
            &self,
            input: GetClientCertificateRequest,
        ) -> Result<ClientCertificate, RusotoError<GetClientCertificateError>>;

        async fn get_client_certificates(
            &self,
            input: GetClientCertificatesRequest,
        ) -> Result<ClientCertificates, RusotoError<GetClientCertificatesError>>;

        async fn get_deployment(
            &self,
            input: GetDeploymentRequest,
        ) -> Result<Deployment, RusotoError<GetDeploymentError>>;

        async fn get_deployments(
            &self,
            input: GetDeploymentsRequest,
        ) -> Result<Deployments, RusotoError<GetDeploymentsError>>;

        async fn get_documentation_part(
            &self,
            input: GetDocumentationPartRequest,
        ) -> Result<DocumentationPart, RusotoError<GetDocumentationPartError>>;

        async fn get_documentation_parts(
            &self,
            input: GetDocumentationPartsRequest,
        ) -> Result<DocumentationParts, RusotoError<GetDocumentationPartsError>>;

        async fn get_documentation_version(
            &self,
            input: GetDocumentationVersionRequest,
        ) -> Result<DocumentationVersion, RusotoError<GetDocumentationVersionError>>;

        async fn get_documentation_versions(
            &self,
            input: GetDocumentationVersionsRequest,
        ) -> Result<DocumentationVersions, RusotoError<GetDocumentationVersionsError>>;

        async fn get_domain_name(
            &self,
            input: GetDomainNameRequest,
        ) -> Result<DomainName, RusotoError<GetDomainNameError>>;

        async fn get_domain_names(
            &self,
            input: GetDomainNamesRequest,
        ) -> Result<DomainNames, RusotoError<GetDomainNamesError>>;

        async fn get_export(
            &self,
            input: GetExportRequest,
        ) -> Result<ExportResponse, RusotoError<GetExportError>>;

        async fn get_gateway_response(
            &self,
            input: GetGatewayResponseRequest,
        ) -> Result<GatewayResponse, RusotoError<GetGatewayResponseError>>;

        async fn get_gateway_responses(
            &self,
            input: GetGatewayResponsesRequest,
        ) -> Result<GatewayResponses, RusotoError<GetGatewayResponsesError>>;

        async fn get_integration(
            &self,
            input: GetIntegrationRequest,
        ) -> Result<Integration, RusotoError<GetIntegrationError>>;

        async fn get_integration_response(
            &self,
            input: GetIntegrationResponseRequest,
        ) -> Result<IntegrationResponse, RusotoError<GetIntegrationResponseError>>;

        async fn get_method(
            &self,
            input: GetMethodRequest,
        ) -> Result<Method, RusotoError<GetMethodError>>;

        async fn get_method_response(
            &self,
            input: GetMethodResponseRequest,
        ) -> Result<MethodResponse, RusotoError<GetMethodResponseError>>;

        async fn get_model(&self, input: GetModelRequest) -> Result<Model, RusotoError<GetModelError>>;

        async fn get_model_template(
            &self,
            input: GetModelTemplateRequest,
        ) -> Result<Template, RusotoError<GetModelTemplateError>>;

        async fn get_models(
            &self,
            input: GetModelsRequest,
        ) -> Result<Models, RusotoError<GetModelsError>>;

        async fn get_request_validator(
            &self,
            input: GetRequestValidatorRequest,
        ) -> Result<RequestValidator, RusotoError<GetRequestValidatorError>>;

        async fn get_request_validators(
            &self,
            input: GetRequestValidatorsRequest,
        ) -> Result<RequestValidators, RusotoError<GetRequestValidatorsError>>;

        async fn get_resource(
            &self,
            input: GetResourceRequest,
        ) -> Result<Resource, RusotoError<GetResourceError>>;

        async fn get_resources(
            &self,
            input: GetResourcesRequest,
        ) -> Result<Resources, RusotoError<GetResourcesError>>;

        async fn get_rest_api(
            &self,
            input: GetRestApiRequest,
        ) -> Result<RestApi, RusotoError<GetRestApiError>>;

        async fn get_rest_apis(
            &self,
            input: GetRestApisRequest,
        ) -> Result<RestApis, RusotoError<GetRestApisError>>;

        async fn get_sdk(&self, input: GetSdkRequest) -> Result<SdkResponse, RusotoError<GetSdkError>>;

        async fn get_sdk_type(
            &self,
            input: GetSdkTypeRequest,
        ) -> Result<SdkType, RusotoError<GetSdkTypeError>>;

        async fn get_sdk_types(
            &self,
            input: GetSdkTypesRequest,
        ) -> Result<SdkTypes, RusotoError<GetSdkTypesError>>;

        async fn get_stage(&self, input: GetStageRequest) -> Result<Stage, RusotoError<GetStageError>>;

        async fn get_stages(
            &self,
            input: GetStagesRequest,
        ) -> Result<Stages, RusotoError<GetStagesError>>;

        async fn get_tags(&self, input: GetTagsRequest) -> Result<Tags, RusotoError<GetTagsError>>;

        async fn get_usage(&self, input: GetUsageRequest) -> Result<Usage, RusotoError<GetUsageError>>;

        async fn get_usage_plan(
            &self,
            input: GetUsagePlanRequest,
        ) -> Result<UsagePlan, RusotoError<GetUsagePlanError>>;

        async fn get_usage_plan_key(
            &self,
            input: GetUsagePlanKeyRequest,
        ) -> Result<UsagePlanKey, RusotoError<GetUsagePlanKeyError>>;

        async fn get_usage_plan_keys(
            &self,
            input: GetUsagePlanKeysRequest,
        ) -> Result<UsagePlanKeys, RusotoError<GetUsagePlanKeysError>>;

        async fn get_usage_plans(
            &self,
            input: GetUsagePlansRequest,
        ) -> Result<UsagePlans, RusotoError<GetUsagePlansError>>;

        async fn get_vpc_link(
            &self,
            input: GetVpcLinkRequest,
        ) -> Result<VpcLink, RusotoError<GetVpcLinkError>>;

        async fn get_vpc_links(
            &self,
            input: GetVpcLinksRequest,
        ) -> Result<VpcLinks, RusotoError<GetVpcLinksError>>;

        async fn import_api_keys(
            &self,
            input: ImportApiKeysRequest,
        ) -> Result<ApiKeyIds, RusotoError<ImportApiKeysError>>;

        async fn import_documentation_parts(
            &self,
            input: ImportDocumentationPartsRequest,
        ) -> Result<DocumentationPartIds, RusotoError<ImportDocumentationPartsError>>;

        async fn import_rest_api(
            &self,
            input: ImportRestApiRequest,
        ) -> Result<RestApi, RusotoError<ImportRestApiError>>;

        async fn put_gateway_response(
            &self,
            input: PutGatewayResponseRequest,
        ) -> Result<GatewayResponse, RusotoError<PutGatewayResponseError>>;

        async fn put_integration(
            &self,
            input: PutIntegrationRequest,
        ) -> Result<Integration, RusotoError<PutIntegrationError>>;

        async fn put_integration_response(
            &self,
            input: PutIntegrationResponseRequest,
        ) -> Result<IntegrationResponse, RusotoError<PutIntegrationResponseError>>;

        async fn put_method(
            &self,
            input: PutMethodRequest,
        ) -> Result<Method, RusotoError<PutMethodError>>;

        async fn put_method_response(
            &self,
            input: PutMethodResponseRequest,
        ) -> Result<MethodResponse, RusotoError<PutMethodResponseError>>;

        async fn put_rest_api(
            &self,
            input: PutRestApiRequest,
        ) -> Result<RestApi, RusotoError<PutRestApiError>>;

        async fn tag_resource(
            &self,
            input: TagResourceRequest,
        ) -> Result<(), RusotoError<TagResourceError>>;

        async fn test_invoke_authorizer(
            &self,
            input: TestInvokeAuthorizerRequest,
        ) -> Result<TestInvokeAuthorizerResponse, RusotoError<TestInvokeAuthorizerError>>;

        async fn test_invoke_method(
            &self,
            input: TestInvokeMethodRequest,
        ) -> Result<TestInvokeMethodResponse, RusotoError<TestInvokeMethodError>>;

        async fn untag_resource(
            &self,
            input: UntagResourceRequest,
        ) -> Result<(), RusotoError<UntagResourceError>>;

        async fn update_account(
            &self,
            input: UpdateAccountRequest,
        ) -> Result<Account, RusotoError<UpdateAccountError>>;

        async fn update_api_key(
            &self,
            input: UpdateApiKeyRequest,
        ) -> Result<ApiKey, RusotoError<UpdateApiKeyError>>;

        async fn update_authorizer(
            &self,
            input: UpdateAuthorizerRequest,
        ) -> Result<Authorizer, RusotoError<UpdateAuthorizerError>>;

        async fn update_base_path_mapping(
            &self,
            input: UpdateBasePathMappingRequest,
        ) -> Result<BasePathMapping, RusotoError<UpdateBasePathMappingError>>;

        async fn update_client_certificate(
            &self,
            input: UpdateClientCertificateRequest,
        ) -> Result<ClientCertificate, RusotoError<UpdateClientCertificateError>>;

        async fn update_deployment(
            &self,
            input: UpdateDeploymentRequest,
        ) -> Result<Deployment, RusotoError<UpdateDeploymentError>>;

        async fn update_documentation_part(
            &self,
            input: UpdateDocumentationPartRequest,
        ) -> Result<DocumentationPart, RusotoError<UpdateDocumentationPartError>>;

        async fn update_documentation_version(
            &self,
            input: UpdateDocumentationVersionRequest,
        ) -> Result<DocumentationVersion, RusotoError<UpdateDocumentationVersionError>>;

        async fn update_domain_name(
            &self,
            input: UpdateDomainNameRequest,
        ) -> Result<DomainName, RusotoError<UpdateDomainNameError>>;

        async fn update_gateway_response(
            &self,
            input: UpdateGatewayResponseRequest,
        ) -> Result<GatewayResponse, RusotoError<UpdateGatewayResponseError>>;

        async fn update_integration(
            &self,
            input: UpdateIntegrationRequest,
        ) -> Result<Integration, RusotoError<UpdateIntegrationError>>;

        async fn update_integration_response(
            &self,
            input: UpdateIntegrationResponseRequest,
        ) -> Result<IntegrationResponse, RusotoError<UpdateIntegrationResponseError>>;

        async fn update_method(
            &self,
            input: UpdateMethodRequest,
        ) -> Result<Method, RusotoError<UpdateMethodError>>;

        async fn update_method_response(
            &self,
            input: UpdateMethodResponseRequest,
        ) -> Result<MethodResponse, RusotoError<UpdateMethodResponseError>>;

        async fn update_model(
            &self,
            input: UpdateModelRequest,
        ) -> Result<Model, RusotoError<UpdateModelError>>;

        async fn update_request_validator(
            &self,
            input: UpdateRequestValidatorRequest,
        ) -> Result<RequestValidator, RusotoError<UpdateRequestValidatorError>>;

        async fn update_resource(
            &self,
            input: UpdateResourceRequest,
        ) -> Result<Resource, RusotoError<UpdateResourceError>>;

        async fn update_rest_api(
            &self,
            input: UpdateRestApiRequest,
        ) -> Result<RestApi, RusotoError<UpdateRestApiError>>;

        async fn update_stage(
            &self,
            input: UpdateStageRequest,
        ) -> Result<Stage, RusotoError<UpdateStageError>>;

        async fn update_usage(
            &self,
            input: UpdateUsageRequest,
        ) -> Result<Usage, RusotoError<UpdateUsageError>>;

        async fn update_usage_plan(
            &self,
            input: UpdateUsagePlanRequest,
        ) -> Result<UsagePlan, RusotoError<UpdateUsagePlanError>>;

        async fn update_vpc_link(
            &self,
            input: UpdateVpcLinkRequest,
        ) -> Result<VpcLink, RusotoError<UpdateVpcLinkError>>;
    }
}
